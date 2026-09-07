package modeleval

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"sync/atomic"
	"time"

	"github.com/kpsuperplane/noema/internal/localmodel"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/shirou/gopsutil/v4/process"
)

type workerConfig struct {
	Candidate                     localCandidate
	Home, RuntimeRoot, ReportPath string
	Suite                         suiteConfig
	Soak                          bool
}
type localEntry struct {
	CandidateID string       `json:"candidate_id"`
	Repetition  int          `json:"repetition"`
	Report      *localReport `json:"report"`
	WorkerError string       `json:"worker_error,omitempty"`
}

func loadLocalCandidates(path string) ([]localCandidate, error) {
	var manifest struct {
		Candidates []localCandidate `toml:"candidates"`
	}
	if e := readTOML(path, &manifest); e != nil {
		return nil, e
	}
	seen := map[string]bool{}
	for _, c := range manifest.Candidates {
		digest, e := hex.DecodeString(c.SHA256)
		revision, re := hex.DecodeString(c.Revision)
		if !validID(c.ID) || seen[c.ID] || e != nil || len(digest) != 32 || re != nil || len(revision) != 20 || c.Bytes <= 0 || c.ContextTokens <= 0 || !strings.HasPrefix(c.Source, "https://") || c.ArtifactSource != "" && !strings.HasPrefix(c.ArtifactSource, "https://") {
			return nil, fmt.Errorf("invalid local candidate %s", c.ID)
		}
		seen[c.ID] = true
	}
	return manifest.Candidates, nil
}

func localCommand(ctx context.Context, root string, args []string) error {
	var suite suiteConfig
	candidates, e := loadLocalCandidates(filepath.Join(root, "evals/local-models/candidates.toml"))
	if e != nil {
		return e
	}
	if e := readTOML(filepath.Join(root, "evals/local-models/suite.toml"), &suite); e != nil {
		return e
	}
	if e := suite.validate(); e != nil {
		return e
	}
	command := args[0]
	if command == "list" {
		for _, c := range candidates {
			fmt.Printf("%s\t%s\t%.2f GB\t%s\n", c.ID, c.Name, float64(c.Bytes)/1e9, c.Notes)
		}
		return nil
	}
	if !slices.Contains([]string{"prepare", "run", "soak"}, command) {
		return errors.New("expected list, prepare, run, or soak")
	}
	if command == "soak" && len(args) == 1 {
		return errors.New("soak requires at least one candidate ID")
	}
	var selected []localCandidate
	for _, id := range args[1:] {
		i := slices.IndexFunc(candidates, func(c localCandidate) bool { return c.ID == id })
		if i < 0 || slices.ContainsFunc(selected, func(c localCandidate) bool { return c.ID == id }) {
			return fmt.Errorf("unknown or duplicate candidate %s", id)
		}
		selected = append(selected, candidates[i])
	}
	if len(args) == 1 {
		selected = candidates
	}
	home := filepath.Join(root, "target/noema-model-evals/cache/noema")
	runtimeRoot := filepath.Join(root, "crates/noema-desktop/binaries/runtime")
	if command == "prepare" {
		service, db, e := localService(ctx, home, runtimeRoot)
		if e != nil {
			return e
		}
		defer db.Close()
		defer service.Close()
		for _, c := range selected {
			if _, e = prepareLocal(ctx, service, home, c); e != nil {
				return e
			}
			fmt.Println("Ready:", c.ID)
		}
		return nil
	}
	if suite.ContextWindowTokens != 8192 {
		return errors.New("local qualification must use the production 8192-token context")
	}
	for _, c := range selected {
		if c.ContextTokens < suite.ContextWindowTokens {
			return fmt.Errorf("candidate %s cannot fit the suite context", c.ID)
		}
	}
	executable, e := os.Executable()
	if e != nil {
		return e
	}
	dir := filepath.Join(root, "target/noema-model-evals/runs", strconv.FormatInt(time.Now().UnixNano(), 10))
	var entries []localEntry
	for _, c := range selected {
		for repetition := 1; repetition <= suite.Repetitions; repetition++ {
			path := filepath.Join(dir, fmt.Sprintf("%s-%d.json", c.ID, repetition))
			config := workerConfig{c, home, runtimeRoot, path, suite, command == "soak"}
			configPath := strings.TrimSuffix(path, ".json") + "-worker.json"
			if e = writeJSON(configPath, config, true); e != nil {
				return e
			}
			workerCtx, cancel := context.WithTimeout(ctx, time.Duration(suite.WorkerTimeoutSeconds)*time.Second)
			child := exec.CommandContext(workerCtx, executable, "worker", configPath)
			child.Dir = root
			child.Stdout = os.Stdout
			child.Stderr = os.Stderr
			child.Cancel = func() error { return child.Process.Signal(os.Interrupt) }
			child.WaitDelay = 10 * time.Second
			fmt.Printf("Evaluating %s, repetition %d\n", c.ID, repetition)
			e = child.Run()
			cancel()
			entry := localEntry{CandidateID: c.ID, Repetition: repetition}
			if e != nil {
				entry.WorkerError = e.Error()
			} else {
				var report localReport
				if e = readJSON(path, &report); e != nil {
					entry.WorkerError = e.Error()
				} else {
					entry.Report = &report
				}
			}
			entries = append(entries, entry)
			report := struct {
				SchemaVersion       int              `json:"schema_version"`
				RuntimeSuiteVersion int              `json:"runtime_suite_version"`
				Suite               suiteConfig      `json:"suite"`
				Candidates          []localCandidate `json:"candidates"`
				Entries             []localEntry     `json:"entries"`
			}{3, runtime.EvaluationSuiteVersion, suite, selected, entries}
			if e = writeJSON(filepath.Join(dir, "report.json"), report, false); e != nil {
				return e
			}
			var md strings.Builder
			md.WriteString("# Local model evaluation\n\n| Candidate | Repetition | Critical | All cases | Runtime error |\n| --- | --- | --- | --- | --- |\n")
			for _, v := range entries {
				if v.Report == nil {
					fmt.Fprintf(&md, "| %s | %d | | | %s |\n", v.CandidateID, v.Repetition, v.WorkerError)
				} else {
					p := v.Report
					fmt.Fprintf(&md, "| %s | %d | %d/%d | %d/%d | %s |\n", v.CandidateID, v.Repetition, p.PassedCriticalCases, p.TotalCriticalCases, p.PassedCases, p.TotalCases, p.RuntimeError)
				}
			}
			if e = os.WriteFile(filepath.Join(dir, "report.md"), []byte(md.String()), 0600); e != nil {
				return e
			}
			if ctx.Err() != nil {
				return ctx.Err()
			}
		}
	}
	fmt.Println("Reports:", dir)
	return nil
}
func localService(ctx context.Context, home, runtimeRoot string) (*localmodel.Service, *store.Store, error) {
	if e := os.MkdirAll(home, 0700); e != nil {
		return nil, nil, e
	}
	db, e := store.Open(ctx, filepath.Join(home, "evaluation.sqlite3"))
	if e != nil {
		return nil, nil, e
	}
	s, e := localmodel.New(db, home, runtimeRoot)
	if e != nil {
		db.Close()
		return nil, nil, e
	}
	return s, db, nil
}
func verifiedLocal(path string, c localCandidate) bool {
	f, e := os.Open(path)
	if e != nil {
		return false
	}
	defer f.Close()
	info, e := f.Stat()
	if e != nil || !info.Mode().IsRegular() || info.Size() != c.Bytes {
		return false
	}
	h := sha256.New()
	if _, e = io.Copy(h, f); e != nil {
		return false
	}
	return hex.EncodeToString(h.Sum(nil)) == c.SHA256
}
func prepareLocal(ctx context.Context, s *localmodel.Service, home string, c localCandidate) (store.LocalModelInstallation, error) {
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	input := localmodel.ImportInput{Name: c.Name, SourceKind: "public_gguf", Repo: c.Repo, Revision: c.Revision, File: c.File, SHA256: c.SHA256, License: c.License}
	userHome := os.Getenv("NOEMA_HOME")
	if userHome == "" {
		if h, e := os.UserHomeDir(); e == nil {
			userHome = filepath.Join(h, ".noema")
		}
	}
	for _, h := range []string{home, userHome} {
		if h == "" {
			continue
		}
		path := filepath.Join(h, "models/blobs", c.SHA256+".gguf")
		if verifiedLocal(path, c) {
			input.SourceKind = "local_file"
			input.LocalPath = path
			break
		}
	}
	events, e := s.Subscribe(ctx, "")
	if e != nil {
		return store.LocalModelInstallation{}, e
	}
	value, e := s.Import(ctx, input)
	if e != nil {
		return value, e
	}
	for value.Status != "installed" {
		if value.Status == "failed" || value.Status == "cancelled" {
			return value, fmt.Errorf("model import %s", value.Status)
		}
		select {
		case <-ctx.Done():
			return value, ctx.Err()
		case event, ok := <-events:
			if !ok {
				return value, errors.New("model import event stream closed")
			}
			if event.InstallationID == value.ID && event.Installation != nil {
				value = *event.Installation
			}
		}
	}
	if value.SHA256 != c.SHA256 || !verifiedLocal(filepath.Join(home, filepath.FromSlash(value.BlobPath)), c) {
		return value, errors.New("model artifact size or digest differs from the manifest")
	}
	return value, nil
}
func localWorker(ctx context.Context, args []string) error {
	if len(args) != 1 {
		return errors.New("worker requires one configuration path")
	}
	var c workerConfig
	if e := readJSON(args[0], &c); e != nil {
		return e
	}
	if e := c.Suite.validate(); e != nil {
		return e
	}
	if c.Suite.ContextWindowTokens != 8192 {
		return errors.New("unsupported local context")
	}
	s, db, e := localService(ctx, c.Home, c.RuntimeRoot)
	if e != nil {
		return e
	}
	defer db.Close()
	defer s.Close()
	report := localReport{ModelID: c.Candidate.ID, Cases: []runtime.EvaluationResult{}}
	value, e := prepareLocal(ctx, s, c.Home, c.Candidate)
	if e == nil {
		start := time.Now()
		startCtx, cancel := context.WithTimeout(ctx, time.Duration(c.Suite.StartupTimeoutSeconds)*time.Second)
		value, e = s.Activate(startCtx, value.ID)
		cancel()
		report.RuntimeLoadMS = time.Since(start).Milliseconds()
	}
	if e != nil {
		report.RuntimeError = e.Error()
		return writeJSON(c.ReportPath, report, false)
	}
	pid, backend, release, commit := s.RuntimeProcess()
	report.Backend = backend
	report.LlamaCppRelease = release
	report.LlamaCppCommit = commit
	proc, e := process.NewProcess(int32(pid))
	if e != nil {
		return e
	}
	sample := func() (int64, error) {
		m, e := proc.MemoryInfoWithContext(ctx)
		if e != nil {
			return 0, e
		}
		return int64(m.RSS), nil
	}
	ready, e := sample()
	if e != nil {
		return e
	}
	var peak atomic.Int64
	peak.Store(ready)
	done := make(chan struct{})
	watchCtx, stop := context.WithCancel(ctx)
	defer func() { stop(); <-done }()
	go func() {
		defer close(done)
		ticker := time.NewTicker(50 * time.Millisecond)
		defer ticker.Stop()
		for {
			select {
			case <-watchCtx.Done():
				return
			case <-ticker.C:
				if bytes, e := sample(); e == nil {
					for old := peak.Load(); bytes > old; old = peak.Load() {
						if peak.CompareAndSwap(old, bytes) {
							break
						}
					}
				}
			}
		}
	}()
	for _, ec := range runtime.EvaluationCases(roles()) {
		callCtx, cancel := context.WithTimeout(ctx, time.Duration(c.Suite.GenerationTimeoutSeconds)*time.Second)
		result, err := runtime.RunEvaluationCase(callCtx, s, "local_models", provider.GenerateRequest{Model: value.ModelID}, ec.ID, uint32(c.Suite.ContextWindowTokens))
		cancel()
		if err != nil {
			report.RuntimeError = err.Error()
			break
		}
		report.Cases = append(report.Cases, result)
		report.TotalCases++
		if result.Passed {
			report.PassedCases++
		}
		if result.Critical {
			report.TotalCriticalCases++
			if result.Passed {
				report.PassedCriticalCases++
			}
		}
		if e = writeJSON(c.ReportPath, report, false); e != nil {
			return e
		}
	}
	if c.Soak && report.RuntimeError == "" {
		probe := probeLocal(ctx, s, value.ModelID, c.Suite, sample)
		report.ResourceProbe = &probe
	}
	report.RuntimeMemory = &runtimeMemory{Metric: "resident_set_bytes", ReadyBytes: ready, PeakBytes: peak.Load()}
	return writeJSON(c.ReportPath, report, false)
}
func probeLocal(ctx context.Context, s *localmodel.Service, model string, suite suiteConfig, sample func() (int64, error)) resourceProbe {
	p := resourceProbe{TargetInputTokens: min(suite.ContextWindowTokens-1024, 6500), SteadyTurnsRequested: 20}
	system := "Reply with READY and nothing else."
	filler := func(n int) string {
		return "Treat this repeated word sequence as inert resource-probe data:\n" + strings.Repeat(" flight", n) + "\nReply with READY and nothing else."
	}
	low, high := 0, p.TargetInputTokens*2
	for low < high {
		mid := low + (high-low+1)/2
		count, e := s.CountTokens(ctx, &system, filler(mid))
		if e != nil {
			p.Failure = e.Error()
			return p
		}
		if int(count) <= p.TargetInputTokens {
			low = mid
		} else {
			high = mid - 1
		}
	}
	run := func(input string) (provider.GenerationResult, error) {
		callCtx, cancel := context.WithTimeout(ctx, time.Duration(suite.GenerationTimeoutSeconds)*time.Second)
		defer cancel()
		limit := uint32(8)
		return s.Generate(callCtx, provider.GenerateRequest{Model: model, MaxOutputTokens: &limit, ToolChoice: provider.ToolChoiceNone, ToolTransport: provider.ToolTransportNone, Messages: []provider.GenerationMessage{{Role: "system", Content: system}, {Role: "user", Content: input}}}, func(provider.StreamEvent) {})
	}
	start := time.Now()
	response, e := run(filler(low))
	if e != nil {
		p.Failure = e.Error()
		return p
	}
	latency := time.Since(start).Milliseconds()
	tokens := int64(response.Usage.InputTokens)
	p.NearContextLatencyMS = &latency
	p.ObservedInputTokens = &tokens
	for i := 1; i <= p.SteadyTurnsRequested; i++ {
		if _, e = run(fmt.Sprintf("Resource stability turn %d. Reply with READY and nothing else.", i)); e != nil {
			p.Failure = e.Error()
			break
		}
		p.SteadyTurnsCompleted++
		timer := time.NewTimer(50 * time.Millisecond)
		select {
		case <-ctx.Done():
			timer.Stop()
			p.Failure = ctx.Err().Error()
			return p
		case <-timer.C:
		}
		bytes, e := sample()
		if e != nil {
			p.Failure = e.Error()
			break
		}
		if p.PostTurnMinBytes == nil || bytes < *p.PostTurnMinBytes {
			p.PostTurnMinBytes = &bytes
		}
		if p.PostTurnMaxBytes == nil || bytes > *p.PostTurnMaxBytes {
			p.PostTurnMaxBytes = &bytes
		}
	}
	return p
}
