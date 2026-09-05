package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/project"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	projectCreateName  = "project.create"
	projectListName    = "project.list"
	projectReadName    = "project.read"
	projectUpdateName  = "project.update"
	projectArchiveName = "project.archive"
	projectReopenName  = "project.reopen"
	projectActorID     = "actor:agent:primary"
)

var projectToolSpecs = []provider.GenerationTool{
	{Name: projectCreateName, Description: "Create a Personal project container and its PROJECT.md.", InputSchema: json.RawMessage(`{"type":"object","properties":{"name":{"type":"string","minLength":1,"maxLength":200},"description":{"type":"string","maxLength":20000},"folder":{"type":"string","minLength":1,"maxLength":4096},"project_document":{"type":"string","maxLength":65536}},"required":["name"],"additionalProperties":false}`)},
	{Name: projectListName, Description: "List bounded Personal projects.", InputSchema: json.RawMessage(`{"type":"object","properties":{"include_archived":{"type":"boolean"},"limit":{"type":"integer","minimum":1,"maximum":100},"cursor":{"type":"string","minLength":1}},"additionalProperties":false}`)},
	{Name: projectReadName, Description: "Read exact project metadata and PROJECT.md content.", InputSchema: json.RawMessage(`{"type":"object","properties":{"project_id":{"type":"string","minLength":1,"maxLength":255}},"required":["project_id"],"additionalProperties":false}`)},
	{Name: projectUpdateName, Description: "Update a project with its revision fence.", InputSchema: json.RawMessage(`{"type":"object","properties":{"project_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"name":{"type":"string","minLength":1},"description":{"type":"string"},"folder":{"type":"string","minLength":1,"maxLength":4096},"clear_folder":{"type":"boolean"}},"required":["project_id","expected_revision"],"additionalProperties":false}`)},
	{Name: projectArchiveName, Description: "Archive a project without changing Task stages.", InputSchema: json.RawMessage(`{"type":"object","properties":{"project_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1}},"required":["project_id","expected_revision"],"additionalProperties":false}`)},
	{Name: projectReopenName, Description: "Reopen an archived project.", InputSchema: json.RawMessage(`{"type":"object","properties":{"project_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1}},"required":["project_id","expected_revision"],"additionalProperties":false}`)},
}

func isProjectTool(name string) bool {
	for _, tool := range projectToolSpecs {
		if tool.Name == name {
			return true
		}
	}
	return false
}

func (c *Chat) executeProjectTool(ctx context.Context, name, requestID, correlationID string, arguments json.RawMessage) (json.RawMessage, bool) {
	if requestID == "" {
		return toolFailure("invalid_input", "Project command key is invalid"), false
	}
	fields, err := strictProjectFields(arguments)
	if err != nil {
		return toolFailure("invalid_input", "Project tool arguments are invalid"), false
	}
	command := project.Command{ActorID: projectActorID, RequestID: requestID, CorrelationID: correlationID}
	switch name {
	case projectCreateName:
		if !onlyProjectFields(fields, "name", "description", "folder", "project_document") {
			return toolFailure("invalid_input", "Project creation arguments are invalid"), false
		}
		nameValue, ok := projectString(fields, "name", true, 200)
		if !ok {
			return toolFailure("invalid_input", "Project name is invalid"), false
		}
		description, ok := projectOptionalString(fields, "description", 20000)
		if !ok {
			return toolFailure("invalid_input", "Project description is invalid"), false
		}
		folder, ok := projectOptionalString(fields, "folder", 4096)
		if !ok || folder != nil && strings.TrimSpace(*folder) == "" {
			return toolFailure("invalid_input", "Project folder is invalid"), false
		}
		document, ok := projectOptionalString(fields, "project_document", 65536)
		if !ok {
			return toolFailure("invalid_input", "Project document is invalid"), false
		}
		result, callErr := c.projects.Create(ctx, project.CreateInput{WorkspaceID: project.PersonalWorkspaceID,
			Name: nameValue, Description: stringValue(description), Folder: folder, Document: document, Command: command})
		if callErr != nil {
			return projectToolError(callErr)
		}
		payload := projectCommandValue(result.ProjectResult)
		payload["document_adopted"] = result.DocumentAdopted
		return marshalProjectTool(payload)
	case projectListName:
		if !onlyProjectFields(fields, "include_archived", "limit", "cursor") {
			return toolFailure("invalid_input", "Project list arguments are invalid"), false
		}
		includeArchived, ok := projectOptionalBool(fields, "include_archived")
		limit, validLimit := projectOptionalInt(fields, "limit", 1, 100)
		cursor, validCursor := projectOptionalString(fields, "cursor", modelToolPayloadLimit)
		if !ok || !validLimit || !validCursor || cursor != nil && *cursor == "" {
			return toolFailure("invalid_input", "Project list arguments are invalid"), false
		}
		if limit == 0 {
			limit = 50
		}
		page, callErr := c.projects.List(ctx, includeArchived, limit, cursor)
		if callErr != nil {
			return projectToolError(callErr)
		}
		values := make([]map[string]any, len(page.Projects))
		for index, value := range page.Projects {
			values[index] = projectValue(value, true)
		}
		return marshalProjectTool(map[string]any{"projects": values,
			"has_next_page": page.HasNextPage, "end_cursor": page.EndCursor})
	case projectReadName:
		if !onlyProjectFields(fields, "project_id") {
			return toolFailure("invalid_input", "Project read arguments are invalid"), false
		}
		projectID, ok := projectString(fields, "project_id", true, 255)
		if !ok {
			return toolFailure("invalid_input", "Project ID is invalid"), false
		}
		value, document, callErr := c.projects.Read(ctx, projectID)
		if callErr != nil {
			return projectToolError(callErr)
		}
		payload := projectValue(value, true)
		payload["project_document"], payload["digest"] = document.Content, document.Digest
		return marshalProjectTool(payload)
	case projectUpdateName:
		if !onlyProjectFields(fields, "project_id", "expected_revision", "name", "description", "folder", "clear_folder") {
			return toolFailure("invalid_input", "Project update arguments are invalid"), false
		}
		projectID, revision, ok := projectFence(fields)
		if !ok {
			return toolFailure("invalid_input", "Project update fence is invalid"), false
		}
		updatedName, nameOK := projectOptionalString(fields, "name", 200)
		description, descriptionOK := projectOptionalString(fields, "description", 20000)
		folder, folderOK := projectOptionalString(fields, "folder", 4096)
		clear, clearOK := projectOptionalBool(fields, "clear_folder")
		if !nameOK || !descriptionOK || !folderOK || !clearOK || updatedName != nil && strings.TrimSpace(*updatedName) == "" {
			return toolFailure("invalid_input", "Project update arguments are invalid"), false
		}
		if clear {
			folder = nil
		}
		result, callErr := c.projects.Update(ctx, project.UpdateInput{ProjectID: projectID,
			ExpectedRevision: int64(revision), Name: updatedName, Description: description,
			Folder: folder, ClearFolder: clear, Command: command})
		if callErr != nil {
			return projectToolError(callErr)
		}
		return marshalProjectTool(projectCommandValue(result.ProjectResult))
	case projectArchiveName, projectReopenName:
		if !onlyProjectFields(fields, "project_id", "expected_revision") {
			return toolFailure("invalid_input", "Project lifecycle arguments are invalid"), false
		}
		projectID, revision, ok := projectFence(fields)
		if !ok {
			return toolFailure("invalid_input", "Project lifecycle fence is invalid"), false
		}
		result, callErr := c.projects.SetArchived(ctx, projectID, int64(revision), name == projectArchiveName, command)
		if callErr != nil {
			return projectToolError(callErr)
		}
		return marshalProjectTool(projectCommandValue(result.ProjectResult))
	default:
		return toolFailure("invalid_input", "Project tool is unavailable"), false
	}
}

func strictProjectFields(raw json.RawMessage) (map[string]json.RawMessage, error) {
	if !utf8.Valid(raw) {
		return nil, errors.New("invalid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	token, err := decoder.Token()
	if err != nil || token != json.Delim('{') {
		return nil, errors.New("arguments are not an object")
	}
	fields := make(map[string]json.RawMessage)
	for decoder.More() {
		keyToken, tokenErr := decoder.Token()
		key, isString := keyToken.(string)
		if tokenErr != nil || !isString {
			return nil, errors.New("invalid argument key")
		}
		if _, exists := fields[key]; exists {
			return nil, errors.New("duplicate argument key")
		}
		var value json.RawMessage
		if err := decoder.Decode(&value); err != nil {
			return nil, err
		}
		fields[key] = value
	}
	if token, err = decoder.Token(); err != nil || token != json.Delim('}') {
		return nil, errors.New("invalid argument object")
	}
	if err := decoder.Decode(&token); err != io.EOF {
		return nil, errors.New("arguments have trailing JSON")
	}
	return fields, nil
}

func projectString(fields map[string]json.RawMessage, key string, required bool, limit int) (string, bool) {
	raw, exists := fields[key]
	if !exists {
		return "", !required
	}
	var value string
	if json.Unmarshal(raw, &value) != nil || len(value) > limit || required && value == "" {
		return "", false
	}
	return value, true
}

func projectOptionalString(fields map[string]json.RawMessage, key string, limit int) (*string, bool) {
	raw, exists := fields[key]
	if !exists {
		return nil, true
	}
	if bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
		return nil, true
	}
	var value string
	if json.Unmarshal(raw, &value) != nil || len(value) > limit {
		return nil, false
	}
	return &value, true
}

func projectOptionalBool(fields map[string]json.RawMessage, key string) (bool, bool) {
	raw, exists := fields[key]
	if !exists {
		return false, true
	}
	if bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
		return false, true
	}
	var value bool
	if json.Unmarshal(raw, &value) != nil {
		return false, false
	}
	return value, true
}

func projectOptionalInt(fields map[string]json.RawMessage, key string, minimum, maximum int) (int, bool) {
	raw, exists := fields[key]
	if !exists {
		return 0, true
	}
	if bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
		return 0, true
	}
	var value int
	if json.Unmarshal(raw, &value) != nil || value < minimum || value > maximum {
		return 0, false
	}
	return value, true
}

func onlyProjectFields(fields map[string]json.RawMessage, names ...string) bool {
	allowed := make(map[string]struct{}, len(names))
	for _, name := range names {
		allowed[name] = struct{}{}
	}
	for name := range fields {
		if _, ok := allowed[name]; !ok {
			return false
		}
	}
	return true
}

func projectFence(fields map[string]json.RawMessage) (string, int, bool) {
	projectID, idOK := projectString(fields, "project_id", true, 255)
	revision, revisionOK := projectOptionalInt(fields, "expected_revision", 1, int(^uint(0)>>1))
	_, hasRevision := fields["expected_revision"]
	return projectID, revision, idOK && revisionOK && hasRevision && revision > 0
}

func stringValue(value *string) string {
	if value == nil {
		return ""
	}
	return *value
}

func projectValue(value store.Project, folder bool) map[string]any {
	result := map[string]any{"project_id": value.ID, "name": value.Name,
		"description": value.Description, "revision": value.Revision, "archived": value.ArchivedAt != nil}
	if folder {
		result["folder"] = value.Folder
	}
	return result
}

func projectCommandValue(result store.ProjectResult) map[string]any {
	return map[string]any{"project": projectValue(result.Project, false),
		"event_id": result.Event.EventID, "event_sequence": result.Event.ID}
}

func marshalProjectTool(value any) (json.RawMessage, bool) {
	payload, err := json.Marshal(value)
	if err != nil {
		return toolFailure("work_unavailable", "Project result is unavailable"), false
	}
	return boundedModelToolPayload(payload, modelToolResultLimit), true
}

func projectToolError(err error) (json.RawMessage, bool) {
	code, message := "work_unavailable", "Project is unavailable"
	switch {
	case errors.Is(err, store.ErrStaleRevision):
		code, message = "stale_revision", "Project revision is stale"
	case errors.Is(err, store.ErrInvalidTransition):
		code, message = "invalid_transition", "Project action is unavailable"
	case errors.Is(err, store.ErrCommandConflict):
		code, message = "command_conflict", "Project command key conflicts with an earlier request"
	case errors.Is(err, store.ErrInvalidCursor):
		code, message = "invalid_cursor", "Project cursor is invalid"
	case errors.Is(err, project.ErrInvalidInput), errors.Is(err, home.ErrInvalidProjectFolder),
		errors.Is(err, home.ErrInvalidProjectDocument), errors.Is(err, home.ErrProjectFolderConflict):
		code, message = "invalid_input", "Project input is invalid"
	}
	return toolFailure(code, message), false
}
