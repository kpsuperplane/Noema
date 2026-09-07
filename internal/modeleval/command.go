package modeleval

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/kpsuperplane/noema/internal/runtime"
)

// Run executes the evaluation CLI from a repository directory.
func Run(ctx context.Context, root string, args []string) error {
	root, e := filepath.Abs(root)
	if e != nil {
		return e
	}
	for {
		if _, e = os.Stat(filepath.Join(root, "go.mod")); e == nil {
			break
		}
		parent := filepath.Dir(root)
		if parent == root {
			return errors.New("run inside the Noema repository")
		}
		root = parent
	}
	if len(args) == 0 {
		args = []string{"list"}
	}
	if args[0] == "worker" {
		return localWorker(ctx, args[1:])
	}
	if args[0] != "matrix" && args[0] != "defaults" {
		return localCommand(ctx, root, args)
	}
	if len(args) < 2 {
		return errors.New("specify list, run, plan, estimate, propose, or verify")
	}
	command := args[1]
	if args[0] == "defaults" && (command == "estimate" || command == "run" || command == "propose" || command == "verify") {
		if len(args) != 3 {
			return fmt.Errorf("defaults %s requires one path", command)
		}
		if command == "propose" || command == "verify" {
			return proposal(root, args[2], command == "verify")
		}
		var p decisionPlan
		if e = readJSON(args[2], &p); e != nil {
			return e
		}
		if e = p.validate(); e != nil {
			return e
		}
		if command == "estimate" {
			fmt.Printf("%s: at most $%.4f across %d candidates and %d repetitions\n", p.DecisionID, p.EstimatedMaxCostUSD, len(p.Candidates), p.Suite.repetitions(true))
			return nil
		}
		if e = p.validateExecutionGitState(root); e != nil {
			return e
		}
		dir := filepath.Join(root, "target/noema-model-evals/decisions", p.DecisionID)
		copyPath := filepath.Join(dir, "plan.json")
		var existing decisionPlan
		if e = readJSON(copyPath, &existing); e == nil {
			if existing.ContentFingerprint != p.ContentFingerprint {
				return errors.New("decision directory contains another plan")
			}
		} else if errors.Is(e, os.ErrNotExist) {
			if e = writeJSON(copyPath, p, true); e != nil {
				return e
			}
		} else {
			return e
		}
		return runMatrix(ctx, root, dir, p.DecisionID, "default_decision", p.Suite, p.Policies, p.Candidates)
	}
	s, p, cs, e := loadMatrix(root, args[2:])
	if e != nil {
		return e
	}
	if args[0] == "defaults" && command == "plan" {
		cost, e := estimate(s, p, cs)
		if e != nil {
			return e
		}
		commit, dirty, e := gitState(root)
		if e != nil {
			return e
		}
		now := time.Now()
		plan := decisionPlan{SchemaVersion: 2, DecisionID: fmt.Sprintf("decision-%d-%s", now.UnixNano(), commit[:12]), CreatedAtUnixSeconds: now.Unix(), GitCommit: commit, GitDirty: dirty, RuntimeSuiteVersion: runtime.EvaluationSuiteVersion, Suite: s, Policies: p, Candidates: cs, EstimatedMaxCostUSD: cost, SpendCeilingUSD: cost}
		plan.ContentFingerprint = plan.hash()
		path := filepath.Join(root, "target/noema-model-evals/plans", plan.DecisionID+".json")
		if e = writeJSON(path, plan, true); e != nil {
			return e
		}
		fmt.Printf("Decision plan: %s\nMaximum estimated cost: $%.4f\n", path, cost)
		if dirty {
			fmt.Println("The plan records a dirty worktree. Create a clean plan before execution.")
		}
		return nil
	}
	if args[0] == "matrix" {
		switch command {
		case "list":
			for _, c := range cs {
				fmt.Printf("%s\t%s\t%v\n", c.ID, c.Model, c.Roles)
			}
			return nil
		case "run":
			mode := "exploration"
			if len(args) == 2 {
				mode = "default_decision"
			}
			id := fmt.Sprintf("matrix-%d", time.Now().UnixNano())
			dir := filepath.Join(root, "target/noema-model-evals", id)
			if e = runMatrix(ctx, root, dir, id, mode, s, p, cs); e != nil {
				return e
			}
			fmt.Println("Reports:", dir)
			return nil
		}
	}
	return errors.New("unknown evaluation command")
}
