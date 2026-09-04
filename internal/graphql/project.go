package graphql

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/vektah/gqlparser/v2/gqlerror"
)

const projectActorID = "actor:human:local"

func (r *Resolver) createProject(ctx context.Context, input model.CreateProjectInput) (*model.ProjectCommandPayload, error) {
	r.projectMu.Lock()
	defer r.projectMu.Unlock()
	if input.WorkspaceID != personalWorkspaceID {
		return nil, projectGraphQLError(errors.New("Project is unavailable"))
	}
	input.Name = strings.TrimSpace(input.Name)
	input.Description = strings.TrimSpace(input.Description)
	input.Folder = trimmedProjectFolder(input.Folder)
	if input.Name == "" || (input.Folder != nil &&
		(!filepath.IsAbs(*input.Folder) || strings.ContainsRune(*input.Folder, 0))) {
		return nil, inputError("Project creation")
	}
	command, err := projectCommand("project.create", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	if replay, found, err := r.Store.LookupProjectReceipt(ctx, command); err != nil || found {
		if found && err == nil {
			_, err = r.recoverProjectStage(replay, command.RequestDigest)
			if err == nil {
				r.Store.NotifyWork()
			}
		}
		return projectPayload(replay, input.ClientMutationID), projectGraphQLError(err)
	}
	projectID, err := store.NewProjectID()
	if err != nil {
		return nil, err
	}
	content := home.DefaultProjectDocument(input.Name, input.Description)
	if input.Folder != nil {
		if existing, readErr := home.ReadProjectDocument(r.home, projectID, input.Folder); readErr == nil {
			content = existing.Content
		} else if !errors.Is(readErr, os.ErrNotExist) {
			return nil, projectGraphQLError(readErr)
		}
	}
	stage, err := home.StageProjectDocument(r.home, projectID, command.RequestDigest, content)
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	result, err := r.Store.CreateProject(ctx, projectID, input.WorkspaceID, input.Name,
		input.Description, input.Folder, stage.Document.Digest, command, time.Now())
	if err != nil {
		recovered, found, reconcileErr := r.reconcileProjectStage(ctx, stage, command)
		if found && reconcileErr == nil {
			r.Store.NotifyWork()
			return projectPayload(recovered, input.ClientMutationID), nil
		}
		return nil, projectGraphQLError(errors.Join(err, reconcileErr))
	}
	if result.Replayed {
		if result.Project.ID != projectID {
			_ = home.DiscardProjectDocumentStage(r.home, projectID, command.RequestDigest)
		}
		if _, err := r.recoverProjectStage(result, command.RequestDigest); err != nil {
			return nil, projectGraphQLError(err)
		}
	} else if _, _, err := home.CommitProjectDocumentStage(r.home, stage, result.Project.Folder); err != nil {
		return nil, projectGraphQLError(err)
	}
	r.Store.NotifyWork()
	return projectPayload(result, input.ClientMutationID), nil
}

func (r *Resolver) updateProject(ctx context.Context, input model.UpdateProjectInput) (*model.ProjectCommandPayload, error) {
	r.projectMu.Lock()
	defer r.projectMu.Unlock()
	input.Name = trimmedValue(input.Name)
	input.Description = trimmedValue(input.Description)
	input.Folder = trimmedProjectFolder(input.Folder)
	if input.ExpectedRevision <= 0 ||
		(input.Name != nil && *input.Name == "") ||
		(input.Folder != nil && (!filepath.IsAbs(*input.Folder) || strings.ContainsRune(*input.Folder, 0))) {
		return nil, inputError("Project update")
	}
	if input.Folder != nil && input.ClearFolder != nil && *input.ClearFolder {
		return nil, inputError("folder and clearFolder")
	}
	changes := store.ProjectChanges{Name: input.Name, Description: input.Description}
	if input.Folder != nil {
		changes.SetFolder, changes.Folder = true, input.Folder
	} else if input.ClearFolder != nil && *input.ClearFolder {
		changes.SetFolder = true
	}
	if changes.Name == nil && changes.Description == nil && !changes.SetFolder {
		return nil, inputError("Project update")
	}
	canonical := struct {
		ProjectID                 string
		ExpectedRevision          int
		Name, Description, Folder *string
		SetFolder                 bool
	}{input.ProjectID, input.ExpectedRevision, input.Name, input.Description, changes.Folder, changes.SetFolder}
	command, err := projectCommand("project.update", input.ClientMutationID, canonical)
	if err != nil {
		return nil, err
	}
	if replay, found, err := r.Store.LookupProjectReceipt(ctx, command); err != nil || found {
		if found && err == nil {
			_, err = r.recoverProjectStage(replay, command.RequestDigest)
			if err == nil {
				r.Store.NotifyWork()
			}
		}
		return projectPayload(replay, input.ClientMutationID), projectGraphQLError(err)
	}
	current, err := r.Store.Project(ctx, input.ProjectID)
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	var stage home.ProjectDocumentStage
	if changes.SetFolder {
		stage, err = home.PrepareProjectDocumentMove(r.home, current.ID, current.Folder,
			changes.Folder, command.RequestDigest)
		if err != nil {
			return nil, projectGraphQLError(err)
		}
		changes.DocumentDigest = stage.Document.Digest
	}
	result, err := r.Store.UpdateProject(ctx, input.ProjectID, int64(input.ExpectedRevision), changes, command, time.Now())
	if err != nil {
		if changes.SetFolder {
			recovered, found, reconcileErr := r.reconcileProjectStage(ctx, stage, command)
			if found && reconcileErr == nil {
				r.Store.NotifyWork()
				return projectPayload(recovered, input.ClientMutationID), nil
			}
			err = errors.Join(err, reconcileErr)
		}
		return nil, projectGraphQLError(err)
	}
	if changes.SetFolder {
		if result.Replayed {
			_, err = r.recoverProjectStage(result, command.RequestDigest)
		} else {
			_, _, err = home.CommitProjectDocumentStage(r.home, stage, result.Project.Folder)
		}
		if err != nil {
			return nil, projectGraphQLError(err)
		}
	}
	r.Store.NotifyWork()
	return projectPayload(result, input.ClientMutationID), nil
}

func (r *Resolver) updateProjectDocument(ctx context.Context, input model.UpdateProjectDocumentInput) (*model.ProjectDocumentCommandPayload, error) {
	r.projectMu.Lock()
	defer r.projectMu.Unlock()
	if input.ExpectedRevision <= 0 || !validDigest(input.ExpectedDocumentDigest) {
		return nil, inputError("Project document update")
	}
	command, err := projectCommand("project.update", input.ClientMutationID, input)
	if err != nil {
		return nil, err
	}
	if replay, found, err := r.Store.LookupProjectReceipt(ctx, command); err != nil {
		return nil, projectGraphQLError(err)
	} else if found {
		if _, recoverErr := r.recoverProjectStage(replay, command.RequestDigest); recoverErr != nil {
			return nil, projectGraphQLError(recoverErr)
		}
		document, readErr := home.ReadProjectDocument(r.home, replay.Project.ID, replay.Project.Folder)
		if readErr == nil {
			r.Store.NotifyWork()
		}
		return projectDocumentPayload(replay, document, input.ClientMutationID), projectGraphQLError(readErr)
	}
	current, err := r.Store.Project(ctx, input.ProjectID)
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	stage, attempted, err := home.PrepareProjectDocumentReplace(r.home, current.ID, current.Folder,
		input.ExpectedDocumentDigest, input.Content, command.RequestDigest)
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	result, err := r.Store.UpdateProject(ctx, input.ProjectID, int64(input.ExpectedRevision),
		store.ProjectChanges{DocumentChanged: true, DocumentDigest: attempted.Digest}, command, time.Now())
	if err != nil {
		recovered, found, reconcileErr := r.reconcileProjectStage(ctx, stage, command)
		if found && reconcileErr == nil {
			document, readErr := home.ReadProjectDocument(r.home, recovered.Project.ID, recovered.Project.Folder)
			if readErr == nil {
				r.Store.NotifyWork()
			}
			return projectDocumentPayload(recovered, document, input.ClientMutationID), projectGraphQLError(readErr)
		}
		return nil, projectGraphQLError(errors.Join(err, reconcileErr))
	}
	if result.Replayed {
		attempted, err = r.recoverProjectStage(result, command.RequestDigest)
	} else {
		attempted, _, err = home.CommitProjectDocumentStage(r.home, stage, result.Project.Folder)
	}
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	r.Store.NotifyWork()
	return projectDocumentPayload(result, attempted, input.ClientMutationID), nil
}

func (r *Resolver) recoverProjectStage(
	result store.ProjectResult,
	requestDigest string,
) (home.ProjectDocument, error) {
	if result.DocumentDigest == "" {
		return home.ProjectDocument{}, nil
	}
	stage, err := home.ReadProjectDocumentStage(r.home, result.Project.ID, requestDigest)
	if errors.Is(err, os.ErrNotExist) {
		document, readErr := home.ReadProjectDocument(r.home, result.Project.ID, result.Project.Folder)
		if readErr != nil {
			return home.ProjectDocument{}, readErr
		}
		if document.Digest != result.DocumentDigest {
			return home.ProjectDocument{}, errors.New("committed Project document does not match its receipt")
		}
		return document, nil
	}
	if err != nil {
		return home.ProjectDocument{}, err
	}
	if stage.Document.Digest != result.DocumentDigest {
		return home.ProjectDocument{}, errors.New("staged Project document does not match its receipt")
	}
	document, _, err := home.CommitProjectDocumentStage(r.home, stage, result.Project.Folder)
	return document, err
}

func (r *Resolver) reconcileProjectStage(
	ctx context.Context,
	stage home.ProjectDocumentStage,
	command store.ProjectCommand,
) (store.ProjectResult, bool, error) {
	replay, found, err := r.Store.LookupProjectReceipt(ctx, command)
	if err != nil {
		if errors.Is(err, store.ErrCommandConflict) {
			discardErr := home.DiscardProjectDocumentStage(r.home, stage.ProjectID, stage.RequestDigest)
			return store.ProjectResult{}, false, errors.Join(err, discardErr)
		}
		return store.ProjectResult{}, false, err
	}
	if !found {
		err := home.DiscardProjectDocumentStage(r.home, stage.ProjectID, stage.RequestDigest)
		return store.ProjectResult{}, false, err
	}
	if replay.Project.ID != stage.ProjectID {
		if err := home.DiscardProjectDocumentStage(r.home, stage.ProjectID, stage.RequestDigest); err != nil {
			return store.ProjectResult{}, true, err
		}
	}
	_, err = r.recoverProjectStage(replay, command.RequestDigest)
	return replay, true, err
}

func (r *Resolver) setProjectArchived(ctx context.Context, projectID string, revision int,
	clientID string, archived bool) (*model.ProjectCommandPayload, error) {
	r.projectMu.Lock()
	defer r.projectMu.Unlock()
	if revision <= 0 {
		return nil, inputError("expectedRevision")
	}
	name := "project.reopen"
	if archived {
		name = "project.archive"
	}
	request := struct {
		ProjectID string `json:"projectId"`
		Revision  int    `json:"expectedRevision"`
	}{projectID, revision}
	command, err := projectCommand(name, clientID, request)
	if err != nil {
		return nil, err
	}
	result, err := r.Store.SetProjectArchived(ctx, projectID, int64(revision), archived, command, time.Now())
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	r.Store.NotifyWork()
	return projectPayload(result, clientID), nil
}

func validDigest(value string) bool {
	if len(value) != 64 || value != strings.ToLower(value) {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func trimmedValue(value *string) *string {
	if value == nil {
		return nil
	}
	trimmed := strings.TrimSpace(*value)
	return &trimmed
}

func trimmedProjectFolder(value *string) *string { return trimmedValue(value) }

func (r *Resolver) projects(ctx context.Context, workspaceID string, includeArchived *bool,
	first *int, after *string) (*model.ProjectConnection, error) {
	r.projectMu.Lock()
	defer r.projectMu.Unlock()
	archived := includeArchived != nil && *includeArchived
	limit := 50
	if first != nil {
		limit = *first
	}
	page, err := r.Store.ListProjects(ctx, workspaceID, archived, limit, after)
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	edges := make([]*model.ProjectEdge, len(page.Projects))
	for index, project := range page.Projects {
		edges[index] = &model.ProjectEdge{Cursor: page.Cursors[index], Node: projectModel(project)}
	}
	return &model.ProjectConnection{Edges: edges, PageInfo: &model.PageInfo{
		EndCursor: page.EndCursor, HasNextPage: page.HasNextPage,
	}}, nil
}

func (r *Resolver) projectDocument(ctx context.Context, projectID string) (*model.ProjectDocument, error) {
	r.projectMu.Lock()
	defer r.projectMu.Unlock()
	project, err := r.Store.Project(ctx, projectID)
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	document, err := home.ReadProjectDocument(r.home, project.ID, project.Folder)
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	return projectDocumentModel(project.ID, document), nil
}

func projectCommand(name, clientID string, input any) (store.ProjectCommand, error) {
	if clientID == "" || clientID != strings.TrimSpace(clientID) {
		return store.ProjectCommand{}, inputError("clientMutationId")
	}
	encoded, err := json.Marshal(input)
	if err != nil {
		return store.ProjectCommand{}, err
	}
	digest := sha256.Sum256(encoded)
	return store.ProjectCommand{ActorID: projectActorID, Name: name,
		ClientMutationID: clientID, RequestDigest: hex.EncodeToString(digest[:])}, nil
}

func projectPayload(result store.ProjectResult, clientID string) *model.ProjectCommandPayload {
	cursor, _ := store.EncodeWorkEventCursor(result.Event.ID)
	return &model.ProjectCommandPayload{Project: projectModel(result.Project),
		EventCursor: cursor, ClientMutationID: clientID}
}

func projectDocumentPayload(result store.ProjectResult, document home.ProjectDocument,
	clientID string) *model.ProjectDocumentCommandPayload {
	cursor, _ := store.EncodeWorkEventCursor(result.Event.ID)
	return &model.ProjectDocumentCommandPayload{Project: projectModel(result.Project),
		Document:    projectDocumentModel(result.Project.ID, document),
		EventCursor: cursor, ClientMutationID: clientID}
}

func projectModel(project store.Project) *model.Project {
	result := &model.Project{ProjectID: project.ID, WorkspaceID: project.WorkspaceID,
		Name: project.Name, Description: project.Description, Folder: project.Folder,
		Revision: int(project.Revision), CreatedAt: project.CreatedAt.Format(time.RFC3339Nano),
		UpdatedAt: project.UpdatedAt.Format(time.RFC3339Nano)}
	if project.ArchivedAt != nil {
		value := project.ArchivedAt.Format(time.RFC3339Nano)
		result.ArchivedAt = &value
	}
	return result
}

func projectDocumentModel(id string, document home.ProjectDocument) *model.ProjectDocument {
	return &model.ProjectDocument{ProjectID: id, Content: document.Content, Digest: document.Digest}
}

func projectGraphQLError(err error) error {
	if err == nil {
		return nil
	}
	code, message := "work_unavailable", "Project is unavailable"
	switch {
	case errors.Is(err, store.ErrStaleRevision):
		code, message = "stale_revision", "the authoritative project revision is stale"
	case errors.Is(err, store.ErrInvalidTransition):
		code, message = "invalid_transition", "the requested Project action is not valid now"
	case errors.Is(err, store.ErrCommandConflict):
		code, message = "idempotency_conflict", "the command key conflicts with an earlier request"
	case errors.Is(err, home.ErrProjectDocumentChanged):
		code, message = "stale_document", "the authoritative Project document changed"
	case errors.Is(err, home.ErrProjectFolderConflict), errors.Is(err, home.ErrInvalidProjectFolder):
		code, message = "invalid_input", "the Project folder is not valid"
	case errors.Is(err, home.ErrInvalidProjectDocument):
		code, message = "invalid_input", "the Project document is not valid"
	case errors.Is(err, store.ErrProjectNotFound):
		code, message = "work_unavailable", "Project is unavailable"
	case errors.Is(err, store.ErrInvalidCursor):
		code, message = "invalid_cursor", "the Project cursor is invalid"
	}
	result := gqlerror.Errorf("%s", message)
	result.Extensions = map[string]any{"code": code}
	return result
}

func inputError(field string) error {
	result := gqlerror.Errorf("invalid %s", field)
	result.Extensions = map[string]any{"code": "invalid_input"}
	return result
}
