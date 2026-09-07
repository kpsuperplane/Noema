// Package localmodel owns one installed llama.cpp runtime and its GGUF files.
package localmodel

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	accountID     = "provider_account:local_models:default"
	contextTokens = uint32(8192)
)

type RuntimeStatus string

const (
	RuntimeInactive RuntimeStatus = "inactive"
	RuntimeStarting RuntimeStatus = "starting"
	RuntimeRunning  RuntimeStatus = "running"
	RuntimeStopping RuntimeStatus = "stopping"
	RuntimeFailed   RuntimeStatus = "failed"
)

type ImportInput struct {
	Name, SourceKind, LocalPath, Repo, Revision, File, SHA256, License string
}

type Event struct {
	Cursor, Kind, InstallationID, ModelID string
	Installation                          *store.LocalModelInstallation
	RuntimeStatus                         RuntimeStatus
	CreatedAt                             time.Time
}

// Service is the single local-model installation and generation authority.
type Service struct {
	database       *store.Store
	home           string
	runtimeRoot    string
	client         *http.Client
	generationMu   sync.Mutex
	operationMu    sync.Mutex
	runtimeOpMu    sync.Mutex
	jobsMu         sync.Mutex
	jobs           map[string]installJob
	jobsWG         sync.WaitGroup
	runtimeMu      sync.Mutex
	runtime        runtimeProcess
	evalMu         sync.Mutex
	eventsMu       sync.Mutex
	subscribers    map[uint64]chan Event
	nextSubscriber uint64
}

type installJob struct {
	cancel context.CancelFunc
	done   chan struct{}
}

func New(database *store.Store, home, runtimeRoot string) (*Service, error) {
	if database == nil || !filepath.IsAbs(home) || runtimeRoot != "" && !filepath.IsAbs(runtimeRoot) {
		return nil, errors.New("local model dependencies are unavailable")
	}
	if runtimeRoot != "" {
		runtimeRoot = filepath.Clean(runtimeRoot)
	}
	client := &http.Client{
		CheckRedirect: func(_ *http.Request, previous []*http.Request) error {
			if len(previous) >= 5 {
				return errors.New("local model download redirected too many times")
			}
			return nil
		},
	}
	return &Service{
		database:    database,
		home:        filepath.Clean(home),
		runtimeRoot: runtimeRoot,
		client:      client,
		jobs:        make(map[string]installJob),
		subscribers: make(map[uint64]chan Event),
	}, nil
}

func (s *Service) Close() {
	s.jobsMu.Lock()
	for _, job := range s.jobs {
		job.cancel()
	}
	s.jobsMu.Unlock()
	s.jobsWG.Wait()
	s.stopRuntime()
}

func (s *Service) Catalog(ctx context.Context) ([]provider.LocalModelCatalogItem, error) {
	return provider.DiscoverLocalModelCatalog(ctx)
}

func (s *Service) Installations(ctx context.Context) ([]store.LocalModelInstallation, error) {
	return s.database.LocalModelInstallations(ctx)
}

func (s *Service) RuntimeStatus() RuntimeStatus {
	s.runtimeMu.Lock()
	defer s.runtimeMu.Unlock()
	return s.runtime.status
}

func (s *Service) Install(ctx context.Context, modelID, file string) (store.LocalModelInstallation, error) {
	catalog, err := s.Catalog(ctx)
	if err != nil {
		return store.LocalModelInstallation{}, err
	}
	var selected *provider.LocalModelCatalogItem
	for i := range catalog {
		if catalog[i].ID == modelID {
			selected = &catalog[i]
			break
		}
	}
	if selected == nil || selected.SelectedBuild == nil || selected.Hardware == nil {
		return store.LocalModelInstallation{}, errors.New("requested local model does not fit this machine")
	}
	build := selected.SelectedBuild
	if file != "" && file != build.File {
		return store.LocalModelInstallation{}, errors.New("requested local model build is unavailable")
	}
	id := "local_model_installation:catalog:" + selected.ID + ":" + build.SHA256[:12]
	value, err := s.database.QueueLocalModel(ctx, store.LocalModelInstallation{ID: id, ModelID: selected.ID, Name: selected.Name, File: build.File, SourceKind: "catalog", Repo: selected.Repo, Revision: selected.Revision, License: selected.License, SHA256: build.SHA256, Backend: string(selected.Hardware.Backend), CreatedAt: time.Now()})
	if err == nil && value.Status != "installed" {
		s.startJob(value, func(job context.Context) error {
			return s.download(job, value, s.huggingFaceURL(value.Repo, value.Revision, value.File), value.SHA256, true)
		})
	}
	return value, err
}

func (s *Service) Import(ctx context.Context, input ImportInput) (store.LocalModelInstallation, error) {
	input.Name = strings.TrimSpace(input.Name)
	if input.Name == "" {
		return store.LocalModelInstallation{}, errors.New("local model name is required")
	}
	backend := s.importBackend(ctx)
	var value store.LocalModelInstallation
	value.Name, value.ModelID, value.SourceKind, value.Backend, value.CreatedAt = input.Name, modelID(input.Name), input.SourceKind, string(backend), time.Now()
	value.License = input.License
	switch input.SourceKind {
	case "public_gguf":
		if err := validateRemote(input.Repo, input.Revision, input.File, input.SHA256); err != nil {
			return value, err
		}
		value.Repo, value.Revision, value.File, value.SHA256 = input.Repo, input.Revision, input.File, input.SHA256
		value.ID = "local_model_installation:public:" + input.SHA256[:12]
	case "local_file":
		info, err := regularGGUF(input.LocalPath)
		if err != nil {
			return value, err
		}
		value.File = filepath.Base(input.LocalPath)
		value.TotalBytes = info.Size()
		value.ID = localFileID(input.LocalPath, info)
	default:
		return value, errors.New("local model import kind is unsupported")
	}
	queued, err := s.database.QueueLocalModel(ctx, value)
	if err != nil {
		return queued, err
	}
	if queued.Status == "installed" {
		return queued, nil
	}
	if input.SourceKind == "public_gguf" {
		s.startJob(queued, func(job context.Context) error {
			return s.download(job, queued, s.huggingFaceURL(input.Repo, input.Revision, input.File), input.SHA256, false)
		})
	} else {
		s.startJob(queued, func(job context.Context) error { return s.copyLocal(job, queued, input.LocalPath) })
	}
	return queued, nil
}

func (s *Service) Cancel(ctx context.Context, id string) (store.LocalModelInstallation, error) {
	s.jobsMu.Lock()
	job := s.jobs[id]
	s.jobsMu.Unlock()
	if job.cancel != nil {
		job.cancel()
		<-job.done
	}
	value, err := s.database.CancelLocalModel(ctx, id, time.Now())
	if err == nil {
		s.publishDurable(ctx)
	}
	return value, err
}

func (s *Service) Remove(ctx context.Context, id string) (bool, error) {
	s.jobsMu.Lock()
	job := s.jobs[id]
	s.jobsMu.Unlock()
	if job.cancel != nil {
		job.cancel()
		<-job.done
	}
	value, err := s.database.RemoveLocalModel(ctx, id, time.Now())
	if err != nil {
		return false, err
	}
	_ = os.Remove(s.partialPath(id))
	if value.BlobPath != "" {
		installations, usageErr := s.database.LocalModelInstallations(ctx)
		if usageErr == nil {
			used := false
			for _, item := range installations {
				used = used || item.BlobPath == value.BlobPath
			}
			if !used {
				_ = os.Remove(filepath.Join(s.home, filepath.FromSlash(value.BlobPath)))
			}
		}
	}
	s.publishDurable(ctx)
	return true, nil
}

func (s *Service) Activate(ctx context.Context, id string) (store.LocalModelInstallation, error) {
	value, err := s.database.LocalModelInstallation(ctx, id)
	if err != nil {
		return value, err
	}
	if err = s.startRuntime(ctx, value); err != nil {
		return value, err
	}
	value, err = s.database.ActivateLocalModel(ctx, id, true, time.Now())
	if err == nil {
		s.publishDurable(ctx)
	}
	return value, err
}

func (s *Service) Retry(ctx context.Context) (RuntimeStatus, error) {
	items, err := s.database.LocalModelInstallations(ctx)
	if err != nil {
		return RuntimeFailed, err
	}
	for _, item := range items {
		if item.Active {
			err = s.startRuntime(ctx, item)
			return s.RuntimeStatus(), err
		}
	}
	return RuntimeInactive, nil
}

func (s *Service) Subscribe(ctx context.Context, after string) (<-chan Event, error) {
	cursor := int64(0)
	if after == "" {
		latest, err := s.database.LatestLocalModelEvent(ctx)
		if err == nil {
			cursor = latest.Cursor
		}
	} else {
		prefix, _, _ := strings.Cut(after, ":")
		var err error
		cursor, err = strconv.ParseInt(prefix, 10, 64)
		if err != nil {
			return nil, errors.New("local model event cursor is invalid")
		}
	}
	live := make(chan Event, 256)
	s.eventsMu.Lock()
	id := s.nextSubscriber
	s.nextSubscriber++
	s.subscribers[id] = live
	s.eventsMu.Unlock()
	var backlog []Event
	for {
		rows, err := s.database.LocalModelEvents(ctx, cursor, 256)
		if err != nil {
			s.unsubscribe(id)
			return nil, err
		}
		for _, row := range rows {
			backlog = append(backlog, s.eventView(ctx, row))
			cursor = row.Cursor
		}
		if len(rows) < 256 {
			break
		}
	}
	output := make(chan Event)
	go func() {
		defer close(output)
		for _, event := range backlog {
			select {
			case output <- event:
			case <-ctx.Done():
				return
			}
		}
		for event := range live {
			select {
			case output <- event:
			case <-ctx.Done():
				return
			}
		}
	}()
	go func() {
		<-ctx.Done()
		s.unsubscribe(id)
	}()
	return output, nil
}

func (s *Service) unsubscribe(id uint64) {
	s.eventsMu.Lock()
	if channel := s.subscribers[id]; channel != nil {
		delete(s.subscribers, id)
		close(channel)
	}
	s.eventsMu.Unlock()
}

func (s *Service) emit(event Event) {
	s.eventsMu.Lock()
	defer s.eventsMu.Unlock()
	for _, channel := range s.subscribers {
		select {
		case channel <- event:
		default:
			select {
			case <-channel:
			default:
			}
			channel <- event
		}
	}
}

func (s *Service) publishDurable(ctx context.Context) {
	row, err := s.database.LatestLocalModelEvent(ctx)
	if err != nil {
		return
	}
	s.emit(s.eventView(ctx, row))
}

func (s *Service) eventView(ctx context.Context, row store.LocalModelEvent) Event {
	event := Event{Cursor: strconv.FormatInt(row.Cursor, 10), Kind: row.Kind, InstallationID: row.InstallationID, ModelID: row.ModelID, CreatedAt: row.CreatedAt}
	if row.InstallationID != "" {
		if value, err := s.database.LocalModelInstallation(ctx, row.InstallationID); err == nil {
			event.Installation = &value
		}
	}
	return event
}

func (s *Service) setRuntimeStatus(status RuntimeStatus) {
	s.runtimeMu.Lock()
	s.runtime.status = status
	s.runtime.sequence++
	sequence := s.runtime.sequence
	s.runtimeMu.Unlock()
	durable := int64(0)
	if latest, err := s.database.LatestLocalModelEvent(context.Background()); err == nil {
		durable = latest.Cursor
	}
	s.emit(Event{Cursor: fmt.Sprintf("%d:runtime:%d", durable, sequence), Kind: "runtime_changed", RuntimeStatus: status, CreatedAt: time.Now().UTC()})
}

func (s *Service) startJob(value store.LocalModelInstallation, work func(context.Context) error) {
	s.jobsMu.Lock()
	if _, exists := s.jobs[value.ID]; exists {
		s.jobsMu.Unlock()
		return
	}
	ctx, cancel := context.WithCancel(context.Background())
	job := installJob{cancel: cancel, done: make(chan struct{})}
	s.jobs[value.ID] = job
	s.jobsWG.Add(1)
	s.jobsMu.Unlock()
	go func() {
		defer s.jobsWG.Done()
		defer func() {
			s.jobsMu.Lock()
			close(job.done)
			delete(s.jobs, value.ID)
			s.jobsMu.Unlock()
		}()
		s.operationMu.Lock()
		defer s.operationMu.Unlock()
		if err := work(ctx); err != nil && !errors.Is(err, context.Canceled) {
			current, loadErr := s.database.LocalModelInstallation(context.Background(), value.ID)
			if loadErr == nil {
				value = current
			}
			_, _ = s.database.UpdateLocalModel(
				context.Background(), value.ID, "failed", value.CompletedBytes, value.TotalBytes,
				0, "", "", installationErrorCode(err), err.Error(), time.Now(),
			)
			s.publishDurable(context.Background())
		}
	}()
}

func (s *Service) importBackend(ctx context.Context) provider.LocalModelBackend {
	profiles, err := provider.LocalModelHardwareProfiles(ctx)
	if err != nil {
		return provider.LocalModelCPU
	}
	return profiles[0].Backend
}
