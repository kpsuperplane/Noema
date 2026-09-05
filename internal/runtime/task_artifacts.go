package runtime

import (
	"context"
	"encoding/json"
	"os"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	artifactCreateLocalName = "artifact.create_local_file"
	taskReadArtifactName    = "task.read_artifact"
	taskListArtifactsName   = "task.list_artifacts"
	taskParseArtifactName   = "task.parse_artifact"
	artifactTextLimit       = 120_000
	artifactReadResultLimit = 1 << 20
)

var (
	artifactCreateLocalSchema = json.RawMessage(`{"type":"object","properties":{"title":{"type":"string","minLength":1,"maxLength":160},"description":{"type":"string","maxLength":1000},"artifact_kind":{"type":"string","minLength":1,"maxLength":80},"filename":{"type":"string","minLength":1,"maxLength":160},"media_type":{"type":"string","maxLength":120},"versions":{"type":"array","minItems":1,"maxItems":5,"items":{"type":"object","properties":{"title":{"type":"string","maxLength":160},"content":{"type":"string","minLength":1,"maxLength":200000}},"required":["content"],"additionalProperties":false}}},"required":["title","artifact_kind","filename","versions"],"additionalProperties":false}`)
	taskReadArtifactSchema    = json.RawMessage(`{"type":"object","properties":{"artifact_id":{"type":"string","minLength":1,"maxLength":200},"artifact_version_id":{"type":"string","minLength":1,"maxLength":200}},"required":["artifact_id"],"additionalProperties":false}`)
	taskListArtifactsSchema   = json.RawMessage(`{"type":"object","properties":{},"additionalProperties":false}`)
	taskParseArtifactSchema   = json.RawMessage(`{"type":"object","properties":{"artifact_id":{"type":"string","minLength":1,"maxLength":200},"artifact_version_id":{"type":"string","minLength":1,"maxLength":200},"max_chars":{"type":"integer","minimum":1000,"maximum":20000}},"required":["artifact_id"],"additionalProperties":false}`)
)

func taskArtifactTools(kind string) []provider.GenerationTool {
	list := provider.GenerationTool{Name: taskListArtifactsName, Description: "List bounded artifacts owned by the current Task.", InputSchema: taskListArtifactsSchema}
	read := provider.GenerationTool{Name: taskReadArtifactName, Description: "Read bounded UTF-8 content from an artifact owned by the current Task.", InputSchema: taskReadArtifactSchema}
	parse := provider.GenerationTool{Name: taskParseArtifactName, Description: "Parse one supported local artifact owned by the current Task.", InputSchema: taskParseArtifactSchema}
	switch kind {
	case "planner":
		return []provider.GenerationTool{list}
	case "executor":
		return []provider.GenerationTool{
			{Name: artifactCreateLocalName, Description: "Create a durable local file artifact owned by the current Task.", InputSchema: artifactCreateLocalSchema},
			list, read, parse,
		}
	case "reviewer":
		return []provider.GenerationTool{list, read, parse}
	default:
		return nil
	}
}

func isTaskArtifactTool(name string) bool {
	return name == artifactCreateLocalName || name == taskReadArtifactName ||
		name == taskListArtifactsName || name == taskParseArtifactName
}

func (r *TaskExecution) executeTaskArtifactTool(ctx context.Context, task store.Task, run store.TaskRun, name string, raw json.RawMessage) (json.RawMessage, bool) {
	if r.artifacts == nil || !taskToolAllowed(run.Kind, name) {
		return toolFailure("unsupported_tool", "Tool is unavailable for this Task role"), false
	}
	current, err := r.database.Task(ctx, task.ID)
	if err != nil || current.Generation != run.Generation || current.CurrentRunID != run.ID {
		return toolFailure("unavailable", "Task artifact context is unavailable"), false
	}
	active, err := r.database.CurrentTaskRun(ctx, current)
	if err != nil || active == nil || active.ID != run.ID || active.Status != "running" || active.Kind != run.Kind {
		return toolFailure("unavailable", "Task artifact context is unavailable"), false
	}
	fields, err := strictProjectFields(raw)
	if err != nil {
		return toolFailure("invalid_input", "Task artifact arguments are invalid"), false
	}
	switch name {
	case artifactCreateLocalName:
		return r.createTaskArtifact(ctx, current, run, fields)
	case taskListArtifactsName:
		if len(fields) != 0 {
			return toolFailure("invalid_input", "Task artifact list arguments are invalid"), false
		}
		values, err := r.database.ArtifactsForOwner(ctx, store.ArtifactOwner{ObjectType: "task", ObjectID: current.ID}, 50)
		if err != nil {
			return toolFailure("unavailable", "Task artifacts are unavailable"), false
		}
		artifacts := make([]map[string]any, len(values))
		for index, value := range values {
			artifacts[index] = artifactValue(value, false)
		}
		payload, _ := json.Marshal(map[string]any{"artifacts": artifacts})
		return boundedModelToolPayload(payload, modelToolResultLimit), true
	case taskReadArtifactName, taskParseArtifactName:
		return r.readTaskArtifact(ctx, current, name, fields)
	default:
		return toolFailure("unsupported_tool", "Tool is unavailable"), false
	}
}

func (r *TaskExecution) createTaskArtifact(ctx context.Context, task store.Task, run store.TaskRun, fields map[string]json.RawMessage) (json.RawMessage, bool) {
	if !onlyProjectFields(fields, "title", "description", "artifact_kind", "filename", "media_type", "versions") {
		return taskArtifactInputFailure()
	}
	title, titleOK := taskString(fields, "title", true, 160)
	kind, kindOK := taskString(fields, "artifact_kind", true, 80)
	filename, filenameOK := taskString(fields, "filename", true, 160)
	description, descriptionOK := taskOptionalString(fields, "description", 1000)
	mediaType, mediaOK := taskOptionalString(fields, "media_type", 120)
	var rawVersions []json.RawMessage
	if raw, exists := fields["versions"]; !exists || json.Unmarshal(raw, &rawVersions) != nil || len(rawVersions) < 1 || len(rawVersions) > 5 ||
		!titleOK || !kindOK || !filenameOK || !descriptionOK || !mediaOK || strings.TrimSpace(title) == "" || strings.TrimSpace(kind) == "" || artifact.SafeFilename(filename) != nil {
		return taskArtifactInputFailure()
	}
	type artifactVersionInput struct {
		Title   *string
		Content string
	}
	versions := make([]artifactVersionInput, 0, len(rawVersions))
	for _, rawVersion := range rawVersions {
		versionFields, parseErr := strictProjectFields(rawVersion)
		if parseErr != nil || !onlyProjectFields(versionFields, "title", "content") {
			return taskArtifactInputFailure()
		}
		content, contentOK := taskString(versionFields, "content", true, 200_000)
		versionTitle, versionTitleOK := taskOptionalString(versionFields, "title", 160)
		if !contentOK || !versionTitleOK || content == "" {
			return taskArtifactInputFailure()
		}
		versions = append(versions, artifactVersionInput{Title: versionTitle, Content: content})
	}
	source := store.ArtifactSource{}
	metadata := map[string]any{"created_by_tool": artifactCreateLocalName, "task_id": task.ID, "task_run_id": run.ID, "filename": filename}
	created, err := r.artifacts.CreateLocal(ctx, artifact.LocalInput{Owner: store.ArtifactOwner{ObjectType: "task", ObjectID: task.ID},
		Title: strings.TrimSpace(title), Description: description, Kind: strings.TrimSpace(kind), Filename: filename,
		Bytes: []byte(versions[0].Content), MediaType: mediaType, CreatedByActorID: run.AgentID, Source: source, Metadata: metadata})
	if err != nil {
		return toolFailure("unavailable", "Task artifact could not be created"), false
	}
	for _, version := range versions[1:] {
		if _, err = r.artifacts.AppendLocal(ctx, created.Artifact.ID, filename, []byte(version.Content), version.Title,
			mediaType, run.AgentID, source, nil); err != nil {
			return toolFailure("unavailable", "Task artifact version could not be created"), false
		}
	}
	created, err = r.database.ArtifactWithVersionsByID(ctx, created.Artifact.ID)
	if err != nil {
		return toolFailure("unavailable", "Task artifact is unavailable"), false
	}
	payload, _ := json.Marshal(artifactValue(created, true))
	return boundedModelToolPayload(payload, modelToolResultLimit), true
}

func (r *TaskExecution) readTaskArtifact(ctx context.Context, task store.Task, name string, fields map[string]json.RawMessage) (json.RawMessage, bool) {
	allowed := []string{"artifact_id", "artifact_version_id"}
	if name == taskParseArtifactName {
		allowed = append(allowed, "max_chars")
	}
	if !onlyProjectFields(fields, allowed...) {
		return taskArtifactInputFailure()
	}
	id, idOK := taskString(fields, "artifact_id", true, 200)
	versionID, versionOK := taskOptionalString(fields, "artifact_version_id", 200)
	if !idOK || !versionOK {
		return taskArtifactInputFailure()
	}
	value, err := r.database.ArtifactWithVersionsByID(ctx, id)
	if err != nil || value.Artifact.Owner != (store.ArtifactOwner{ObjectType: "task", ObjectID: task.ID}) {
		return toolFailure("unavailable", "Artifact is unavailable"), false
	}
	version := value.CurrentVersion
	if versionID != nil {
		found := false
		for _, candidate := range value.Versions {
			if candidate.ID == *versionID {
				version, found = candidate, true
				break
			}
		}
		if !found {
			return toolFailure("unavailable", "Artifact version is unavailable"), false
		}
	}
	file, err := r.artifacts.Read(value.Artifact, version)
	if err != nil {
		return toolFailure("unavailable", "Artifact content is unavailable"), false
	}
	if name == taskReadArtifactName {
		if !utf8.Valid(file.Bytes) || utf8.RuneCount(file.Bytes) > artifactTextLimit {
			return toolFailure("invalid_input", "Artifact is not bounded UTF-8 text"), false
		}
		payload := artifactValue(value, false)
		payload["artifact_version_id"], payload["version_index"] = version.ID, version.Index
		payload["version_metadata"], payload["content"] = version.Metadata, string(file.Bytes)
		encoded, _ := json.Marshal(payload)
		return boundedModelToolPayload(encoded, artifactReadResultLimit), true
	}
	maxChars, ok := projectOptionalInt(fields, "max_chars", 1000, 20000)
	if !ok {
		return taskArtifactInputFailure()
	}
	if maxChars == 0 {
		maxChars = 12000
	}
	temporary, err := os.CreateTemp("", "noema-artifact-parse-*")
	if err != nil {
		return toolFailure("unavailable", "Artifact parser is unavailable"), false
	}
	path := temporary.Name()
	defer os.Remove(path)
	defer temporary.Close()
	if _, err = temporary.Write(file.Bytes); err != nil {
		return toolFailure("unavailable", "Artifact parser is unavailable"), false
	}
	if _, err = temporary.Seek(0, 0); err != nil {
		return toolFailure("unavailable", "Artifact parser is unavailable"), false
	}
	parsed := parseOpenFileWithMedia(ctx, temporary, file.Filename, maxChars, file.MediaType)
	payload := artifactValue(value, false)
	payload["artifact_version_id"], payload["version_index"], payload["parse"] = version.ID, version.Index, parsed
	encoded, _ := json.Marshal(payload)
	return boundedModelToolPayload(encoded, modelToolResultLimit), true
}

func artifactValue(value store.ArtifactWithVersions, includeVersions bool) map[string]any {
	result := map[string]any{"artifact_id": value.Artifact.ID, "title": value.Artifact.Title,
		"artifact_kind": value.Artifact.Kind, "metadata": value.Artifact.Metadata,
		"artifact_version_id": value.CurrentVersion.ID, "version_index": value.CurrentVersion.Index,
		"media_type": value.CurrentVersion.MediaType, "byte_size": value.CurrentVersion.ByteSize}
	if includeVersions {
		result["storage_kind"], result["current_version_id"], result["current_version_index"] = value.Artifact.StorageKind, value.CurrentVersion.ID, value.CurrentVersion.Index
		result["download_url"] = artifact.DownloadURL(value.CurrentVersion.ID)
		versions := make([]map[string]any, len(value.Versions))
		for index, version := range value.Versions {
			versions[index] = map[string]any{"artifact_version_id": version.ID, "version_index": version.Index,
				"download_url": artifact.DownloadURL(version.ID), "media_type": version.MediaType,
				"byte_size": version.ByteSize, "metadata": version.Metadata}
		}
		result["versions"] = versions
	}
	return result
}

func taskArtifactInputFailure() (json.RawMessage, bool) {
	return toolFailure("invalid_input", "Task artifact arguments are invalid"), false
}
