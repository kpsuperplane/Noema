// Package project owns Project metadata and PROJECT.md publication.
package project

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

const PersonalWorkspaceID = "workspace:personal"

var ErrInvalidInput = errors.New("invalid Project input")

type Service struct {
	database *store.Store
	root     *os.Root
	mu       sync.Mutex
}

type Command struct {
	ActorID, RequestID, CorrelationID string
}

type CreateInput struct {
	WorkspaceID, Name, Description string
	Folder                         *string
	Document                       *string
	Command                        Command
}

type UpdateInput struct {
	ProjectID         string
	ExpectedRevision  int64
	Name, Description *string
	Folder            *string
	ClearFolder       bool
	Command           Command
}

type DocumentInput struct {
	ProjectID, ExpectedDigest, Content string
	ExpectedRevision                   int64
	Command                            Command
}

type Result struct {
	store.ProjectResult
	Document        home.ProjectDocument
	DocumentAdopted bool
}

func New(database *store.Store, root *os.Root) *Service {
	return &Service{database: database, root: root}
}

func (s *Service) Create(ctx context.Context, input CreateInput) (Result, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	input.Name = strings.TrimSpace(input.Name)
	input.Description = strings.TrimSpace(input.Description)
	input.Folder = trimmed(input.Folder)
	if input.WorkspaceID != PersonalWorkspaceID || input.Name == "" ||
		input.Folder != nil && !validFolder(*input.Folder) {
		return Result{}, ErrInvalidInput
	}
	canonical := struct {
		WorkspaceID      string  `json:"workspaceId"`
		Name             string  `json:"name"`
		Description      string  `json:"description"`
		Folder           *string `json:"folder,omitempty"`
		ClientMutationID string  `json:"clientMutationId"`
		Document         *string `json:"projectDocument,omitempty"`
	}{input.WorkspaceID, input.Name, input.Description, input.Folder, input.Command.RequestID, input.Document}
	command, err := projectCommand("project.create", input.Command, canonical)
	if err != nil {
		return Result{}, err
	}
	if replay, found, err := s.database.LookupProjectReceipt(ctx, command); err != nil || found {
		if found && err == nil {
			var document home.ProjectDocument
			document, err = s.recover(replay, command.RequestDigest)
			if err == nil {
				s.database.NotifyWork()
			}
			return Result{ProjectResult: replay, Document: document,
				DocumentAdopted: replay.DocumentAdopted}, err
		}
		return Result{}, err
	}
	projectID, err := store.NewProjectID()
	if err != nil {
		return Result{}, err
	}
	content := home.DefaultProjectDocument(input.Name, input.Description)
	if input.Document != nil {
		content = *input.Document
	}
	adopted := false
	if input.Folder != nil {
		if existing, readErr := home.ReadProjectDocument(s.root, projectID, input.Folder); readErr == nil {
			content, adopted = existing.Content, true
		} else if !errors.Is(readErr, os.ErrNotExist) {
			return Result{}, readErr
		}
	}
	stage, err := home.StageProjectDocument(s.root, projectID, command.RequestDigest, content)
	if err != nil {
		return Result{}, err
	}
	stored, err := s.database.CreateProject(ctx, projectID, input.WorkspaceID, input.Name,
		input.Description, input.Folder, stage.Document.Digest, adopted, command, time.Now())
	if err != nil {
		recovered, found, reconcileErr := s.reconcile(ctx, stage, command)
		if found && reconcileErr == nil {
			s.database.NotifyWork()
			return Result{ProjectResult: recovered, Document: stage.Document,
				DocumentAdopted: recovered.DocumentAdopted}, nil
		}
		return Result{}, errors.Join(err, reconcileErr)
	}
	document := stage.Document
	if stored.Replayed {
		if stored.Project.ID != projectID {
			_ = home.DiscardProjectDocumentStage(s.root, projectID, command.RequestDigest)
		}
		document, err = s.recover(stored, command.RequestDigest)
	} else {
		document, _, err = home.CommitProjectDocumentStage(s.root, stage, stored.Project.Folder)
	}
	if err != nil {
		return Result{}, err
	}
	s.database.NotifyWork()
	return Result{ProjectResult: stored, Document: document,
		DocumentAdopted: stored.DocumentAdopted}, nil
}

func (s *Service) Update(ctx context.Context, input UpdateInput) (Result, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	input.Name, input.Description, input.Folder = trimmed(input.Name), trimmed(input.Description), trimmed(input.Folder)
	if input.ExpectedRevision <= 0 || input.Name != nil && *input.Name == "" ||
		input.Folder != nil && !validFolder(*input.Folder) || input.Folder != nil && input.ClearFolder {
		return Result{}, ErrInvalidInput
	}
	changes := store.ProjectChanges{Name: input.Name, Description: input.Description}
	if input.Folder != nil {
		changes.SetFolder, changes.Folder = true, input.Folder
	} else if input.ClearFolder {
		changes.SetFolder = true
	}
	if changes.Name == nil && changes.Description == nil && !changes.SetFolder {
		return Result{}, ErrInvalidInput
	}
	canonical := struct {
		ProjectID                 string
		ExpectedRevision          int64
		Name, Description, Folder *string
		SetFolder                 bool
	}{input.ProjectID, input.ExpectedRevision, input.Name, input.Description, changes.Folder, changes.SetFolder}
	command, err := projectCommand("project.update", input.Command, canonical)
	if err != nil {
		return Result{}, err
	}
	if replay, found, err := s.database.LookupProjectReceipt(ctx, command); err != nil || found {
		if found && err == nil {
			_, err = s.recover(replay, command.RequestDigest)
			if err == nil {
				s.database.NotifyWork()
			}
			return Result{ProjectResult: replay}, err
		}
		return Result{}, err
	}
	current, err := s.database.Project(ctx, input.ProjectID)
	if err != nil {
		return Result{}, err
	}
	var stage home.ProjectDocumentStage
	if changes.SetFolder {
		stage, err = home.PrepareProjectDocumentMove(s.root, current.ID, current.Folder, changes.Folder, command.RequestDigest)
		if err != nil {
			return Result{}, err
		}
		changes.DocumentDigest = stage.Document.Digest
	}
	stored, err := s.database.UpdateProject(ctx, input.ProjectID, input.ExpectedRevision, changes, command, time.Now())
	if err != nil {
		if changes.SetFolder {
			recovered, found, reconcileErr := s.reconcile(ctx, stage, command)
			if found && reconcileErr == nil {
				s.database.NotifyWork()
				return Result{ProjectResult: recovered}, nil
			}
			err = errors.Join(err, reconcileErr)
		}
		return Result{}, err
	}
	if changes.SetFolder {
		if stored.Replayed {
			_, err = s.recover(stored, command.RequestDigest)
		} else {
			_, _, err = home.CommitProjectDocumentStage(s.root, stage, stored.Project.Folder)
		}
		if err != nil {
			return Result{}, err
		}
	}
	s.database.NotifyWork()
	return Result{ProjectResult: stored}, nil
}

func (s *Service) UpdateDocument(ctx context.Context, input DocumentInput) (Result, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if input.ExpectedRevision <= 0 || !validDigest(input.ExpectedDigest) {
		return Result{}, ErrInvalidInput
	}
	canonical := struct {
		ProjectID              string `json:"projectId"`
		ExpectedRevision       int64  `json:"expectedRevision"`
		ExpectedDocumentDigest string `json:"expectedDocumentDigest"`
		Content                string `json:"content"`
		ClientMutationID       string `json:"clientMutationId"`
	}{input.ProjectID, input.ExpectedRevision, input.ExpectedDigest, input.Content, input.Command.RequestID}
	command, err := projectCommand("project.update", input.Command, canonical)
	if err != nil {
		return Result{}, err
	}
	if replay, found, err := s.database.LookupProjectReceipt(ctx, command); err != nil {
		return Result{}, err
	} else if found {
		document, recoverErr := s.recover(replay, command.RequestDigest)
		if recoverErr == nil {
			s.database.NotifyWork()
		}
		return Result{ProjectResult: replay, Document: document}, recoverErr
	}
	current, err := s.database.Project(ctx, input.ProjectID)
	if err != nil {
		return Result{}, err
	}
	stage, attempted, err := home.PrepareProjectDocumentReplace(s.root, current.ID, current.Folder,
		input.ExpectedDigest, input.Content, command.RequestDigest)
	if err != nil {
		return Result{}, err
	}
	stored, err := s.database.UpdateProject(ctx, input.ProjectID, input.ExpectedRevision,
		store.ProjectChanges{DocumentChanged: true, DocumentDigest: attempted.Digest}, command, time.Now())
	if err != nil {
		recovered, found, reconcileErr := s.reconcile(ctx, stage, command)
		if found && reconcileErr == nil {
			document, readErr := home.ReadProjectDocument(s.root, recovered.Project.ID, recovered.Project.Folder)
			if readErr == nil {
				s.database.NotifyWork()
			}
			return Result{ProjectResult: recovered, Document: document}, readErr
		}
		return Result{}, errors.Join(err, reconcileErr)
	}
	if stored.Replayed {
		attempted, err = s.recover(stored, command.RequestDigest)
	} else {
		attempted, _, err = home.CommitProjectDocumentStage(s.root, stage, stored.Project.Folder)
	}
	if err != nil {
		return Result{}, err
	}
	s.database.NotifyWork()
	return Result{ProjectResult: stored, Document: attempted}, nil
}

func (s *Service) SetArchived(ctx context.Context, projectID string, revision int64, archived bool, request Command) (Result, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if revision <= 0 {
		return Result{}, ErrInvalidInput
	}
	name := "project.reopen"
	if archived {
		name = "project.archive"
	}
	command, err := projectCommand(name, request, struct {
		ProjectID        string `json:"projectId"`
		ExpectedRevision int64  `json:"expectedRevision"`
	}{projectID, revision})
	if err != nil {
		return Result{}, err
	}
	stored, err := s.database.SetProjectArchived(ctx, projectID, revision, archived, command, time.Now())
	if err == nil {
		s.database.NotifyWork()
	}
	return Result{ProjectResult: stored}, err
}

func (s *Service) List(ctx context.Context, includeArchived bool, limit int, after *string) (store.ProjectPage, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.database.ListProjects(ctx, PersonalWorkspaceID, includeArchived, limit, after)
}

func (s *Service) Read(ctx context.Context, projectID string) (store.Project, home.ProjectDocument, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	value, err := s.database.Project(ctx, projectID)
	if err != nil {
		return store.Project{}, home.ProjectDocument{}, err
	}
	document, err := home.ReadProjectDocument(s.root, value.ID, value.Folder)
	return value, document, err
}

func (s *Service) recover(result store.ProjectResult, requestDigest string) (home.ProjectDocument, error) {
	if result.DocumentDigest == "" {
		return home.ProjectDocument{}, nil
	}
	stage, err := home.ReadProjectDocumentStage(s.root, result.Project.ID, requestDigest)
	if errors.Is(err, os.ErrNotExist) {
		document, readErr := home.ReadProjectDocument(s.root, result.Project.ID, result.Project.Folder)
		if readErr != nil {
			return home.ProjectDocument{}, readErr
		}
		if document.Digest != result.DocumentDigest {
			return home.ProjectDocument{}, errors.New("committed Project document does not match its receipt")
		}
		return document, nil
	}
	if err != nil || stage.Document.Digest != result.DocumentDigest {
		if err == nil {
			err = errors.New("staged Project document does not match its receipt")
		}
		return home.ProjectDocument{}, err
	}
	document, _, err := home.CommitProjectDocumentStage(s.root, stage, result.Project.Folder)
	return document, err
}

func (s *Service) reconcile(ctx context.Context, stage home.ProjectDocumentStage, command store.ProjectCommand) (store.ProjectResult, bool, error) {
	replay, found, err := s.database.LookupProjectReceipt(ctx, command)
	if err != nil {
		if errors.Is(err, store.ErrCommandConflict) {
			err = errors.Join(err, home.DiscardProjectDocumentStage(s.root, stage.ProjectID, stage.RequestDigest))
		}
		return store.ProjectResult{}, false, err
	}
	if !found {
		return store.ProjectResult{}, false, home.DiscardProjectDocumentStage(s.root, stage.ProjectID, stage.RequestDigest)
	}
	if replay.Project.ID != stage.ProjectID {
		if err := home.DiscardProjectDocumentStage(s.root, stage.ProjectID, stage.RequestDigest); err != nil {
			return store.ProjectResult{}, true, err
		}
	}
	_, err = s.recover(replay, command.RequestDigest)
	return replay, true, err
}

func projectCommand(name string, request Command, input any) (store.ProjectCommand, error) {
	if request.RequestID == "" || request.RequestID != strings.TrimSpace(request.RequestID) ||
		!strings.HasPrefix(request.ActorID, "actor:") {
		return store.ProjectCommand{}, ErrInvalidInput
	}
	encoded, err := json.Marshal(input)
	if err != nil {
		return store.ProjectCommand{}, err
	}
	digest := sha256.Sum256(encoded)
	return store.ProjectCommand{ActorID: request.ActorID, Name: name,
		ClientMutationID: request.RequestID, RequestDigest: hex.EncodeToString(digest[:]),
		CorrelationID: request.CorrelationID}, nil
}

func trimmed(value *string) *string {
	if value == nil {
		return nil
	}
	clean := strings.TrimSpace(*value)
	return &clean
}

func validFolder(value string) bool {
	return filepath.IsAbs(value) && !strings.ContainsRune(value, 0)
}

func validDigest(value string) bool {
	if len(value) != 64 || value != strings.ToLower(value) {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}
