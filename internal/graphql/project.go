package graphql

import (
	"context"
	"errors"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/project"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/vektah/gqlparser/v2/gqlerror"
)

const projectActorID = "actor:human:local"

func (r *Resolver) createProject(ctx context.Context, input model.CreateProjectInput) (*model.ProjectCommandPayload, error) {
	result, err := r.Projects.Create(ctx, project.CreateInput{
		WorkspaceID: input.WorkspaceID, Name: input.Name, Description: input.Description,
		Folder: input.Folder, Command: project.Command{ActorID: projectActorID, RequestID: input.ClientMutationID},
	})
	return projectPayload(result.ProjectResult, input.ClientMutationID), projectGraphQLError(err)
}

func (r *Resolver) updateProject(ctx context.Context, input model.UpdateProjectInput) (*model.ProjectCommandPayload, error) {
	result, err := r.Projects.Update(ctx, project.UpdateInput{
		ProjectID: input.ProjectID, ExpectedRevision: int64(input.ExpectedRevision), Name: input.Name,
		Description: input.Description, Folder: input.Folder,
		ClearFolder: input.ClearFolder != nil && *input.ClearFolder,
		Command:     project.Command{ActorID: projectActorID, RequestID: input.ClientMutationID},
	})
	return projectPayload(result.ProjectResult, input.ClientMutationID), projectGraphQLError(err)
}

func (r *Resolver) updateProjectDocument(ctx context.Context, input model.UpdateProjectDocumentInput) (*model.ProjectDocumentCommandPayload, error) {
	result, err := r.Projects.UpdateDocument(ctx, project.DocumentInput{
		ProjectID: input.ProjectID, ExpectedRevision: int64(input.ExpectedRevision),
		ExpectedDigest: input.ExpectedDocumentDigest, Content: input.Content,
		Command: project.Command{ActorID: projectActorID, RequestID: input.ClientMutationID},
	})
	return projectDocumentPayload(result.ProjectResult, result.Document, input.ClientMutationID), projectGraphQLError(err)
}

func (r *Resolver) setProjectArchived(ctx context.Context, projectID string, revision int,
	clientID string, archived bool) (*model.ProjectCommandPayload, error) {
	result, err := r.Projects.SetArchived(ctx, projectID, int64(revision), archived,
		project.Command{ActorID: projectActorID, RequestID: clientID})
	return projectPayload(result.ProjectResult, clientID), projectGraphQLError(err)
}

func (r *Resolver) projects(ctx context.Context, workspaceID string, includeArchived *bool,
	first *int, after *string) (*model.ProjectConnection, error) {
	if workspaceID != project.PersonalWorkspaceID {
		return nil, projectGraphQLError(errors.New("Project is unavailable"))
	}
	archived, limit := includeArchived != nil && *includeArchived, 50
	if first != nil {
		limit = *first
	}
	if limit < 1 || limit > 100 {
		return nil, projectGraphQLError(store.ErrInvalidCursor)
	}
	page, err := r.Projects.List(ctx, archived, limit, after)
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	edges := make([]*model.ProjectEdge, len(page.Projects))
	for index, value := range page.Projects {
		edges[index] = &model.ProjectEdge{Cursor: page.Cursors[index], Node: projectModel(value)}
	}
	return &model.ProjectConnection{Edges: edges, PageInfo: &model.PageInfo{
		EndCursor: page.EndCursor, HasNextPage: page.HasNextPage,
	}}, nil
}

func (r *Resolver) projectDocument(ctx context.Context, projectID string) (*model.ProjectDocument, error) {
	value, document, err := r.Projects.Read(ctx, projectID)
	if err != nil {
		return nil, projectGraphQLError(err)
	}
	return projectDocumentModel(value.ID, document), nil
}

func projectPayload(result store.ProjectResult, clientID string) *model.ProjectCommandPayload {
	if result.Project.ID == "" {
		return nil
	}
	cursor, _ := store.EncodeWorkEventCursor(result.Event.ID)
	return &model.ProjectCommandPayload{Project: projectModel(result.Project),
		EventCursor: cursor, ClientMutationID: clientID}
}

func projectDocumentPayload(result store.ProjectResult, document home.ProjectDocument,
	clientID string) *model.ProjectDocumentCommandPayload {
	if result.Project.ID == "" {
		return nil
	}
	cursor, _ := store.EncodeWorkEventCursor(result.Event.ID)
	return &model.ProjectDocumentCommandPayload{Project: projectModel(result.Project),
		Document:    projectDocumentModel(result.Project.ID, document),
		EventCursor: cursor, ClientMutationID: clientID}
}

func projectModel(value store.Project) *model.Project {
	result := &model.Project{ProjectID: value.ID, WorkspaceID: value.WorkspaceID,
		Name: value.Name, Description: value.Description, Folder: value.Folder,
		Revision: int(value.Revision), CreatedAt: value.CreatedAt.Format(time.RFC3339Nano),
		UpdatedAt: value.UpdatedAt.Format(time.RFC3339Nano)}
	if value.ArchivedAt != nil {
		archivedAt := value.ArchivedAt.Format(time.RFC3339Nano)
		result.ArchivedAt = &archivedAt
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
	case errors.Is(err, home.ErrProjectFolderConflict), errors.Is(err, home.ErrInvalidProjectFolder),
		errors.Is(err, home.ErrInvalidProjectDocument), errors.Is(err, project.ErrInvalidInput):
		code, message = "invalid_input", "the Project input is not valid"
	case errors.Is(err, store.ErrProjectNotFound):
		code, message = "work_unavailable", "Project is unavailable"
	case errors.Is(err, store.ErrInvalidCursor):
		code, message = "invalid_cursor", "invalid task cursor"
	}
	result := gqlerror.Errorf("%s", message)
	result.Extensions = map[string]any{"code": code}
	return result
}
