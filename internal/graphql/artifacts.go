package graphql

import (
	"context"
	"encoding/base64"
	"errors"
	"math"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/documents"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

const maxTaskArtifactUpload = 40 * 1024

func (r *Resolver) createConversationExternalArtifact(
	ctx context.Context,
	input model.CreateConversationExternalArtifactInput,
) (*model.Artifact, error) {
	if r.Artifacts == nil {
		return nil, errors.New("Artifact service is unavailable")
	}
	if _, err := r.Store.Conversation(ctx, input.ConversationID); err != nil {
		return nil, errors.New("conversation is unavailable")
	}
	created, err := r.Artifacts.CreateExternal(ctx, artifact.ExternalInput{
		Owner: store.ArtifactOwner{ObjectType: "conversation", ObjectID: input.ConversationID},
		Title: input.Title, Description: input.Description, Kind: input.ArtifactKind,
		URL: input.ExternalURL, MediaType: input.MediaType, CreatedByActorID: "human:local",
		Source: store.ArtifactSource{ConversationID: input.ConversationID},
	})
	if err != nil {
		return nil, err
	}
	return artifactModel(created)
}

func (r *Resolver) createTaskLocalArtifact(
	ctx context.Context,
	input model.CreateTaskLocalArtifactInput,
) (*model.Artifact, error) {
	if r.Artifacts == nil {
		return nil, errors.New("Artifact service is unavailable")
	}
	task, err := r.Store.Task(ctx, input.TaskID)
	if err != nil || task.State != store.TaskCaptured || task.Revision != int64(input.ExpectedRevision) || input.ExpectedGeneration != 1 {
		return nil, errors.New("Task is unavailable")
	}
	if !validArtifactLabel(input.Title, 160) {
		return nil, errors.New("invalid title")
	}
	if !validArtifactLabel(input.MediaType, 120) {
		return nil, errors.New("invalid mediaType")
	}
	if err := artifact.SafeFilename(input.Filename); err != nil {
		return nil, errors.New("invalid filename")
	}
	bytes, err := base64.StdEncoding.DecodeString(input.ContentBase64)
	if err != nil || len(bytes) == 0 || len(bytes) > maxTaskArtifactUpload {
		return nil, errors.New("invalid contentBase64")
	}
	description := "Private Task source file"
	created, err := r.Artifacts.CreateLocal(ctx, artifact.LocalInput{
		Owner: store.ArtifactOwner{ObjectType: "task", ObjectID: task.ID},
		Title: input.Title, Description: &description, Kind: "source_file",
		Filename: input.Filename, Bytes: bytes, MediaType: &input.MediaType,
		CreatedByActorID: "human:local",
	})
	if err != nil {
		return nil, err
	}
	return artifactModel(created)
}

func (r *Resolver) artifacts(
	ctx context.Context,
	ownerType, ownerID string,
	limit *int,
) ([]*model.Artifact, error) {
	owner := store.ArtifactOwner{ObjectType: ownerType, ObjectID: ownerID}
	authorized, err := r.Store.ArtifactOwnerAuthorized(ctx, owner)
	if err != nil {
		return nil, err
	}
	if !authorized {
		return []*model.Artifact{}, nil
	}
	count := 20
	if limit != nil {
		count = *limit
	}
	stored, err := r.Store.ArtifactsForOwner(ctx, owner, count)
	if err != nil {
		return nil, err
	}
	result := make([]*model.Artifact, 0, len(stored))
	for _, value := range stored {
		mapped, err := artifactModel(value)
		if err != nil {
			return nil, err
		}
		result = append(result, mapped)
	}
	return result, nil
}

func (r *Resolver) artifactVersionDetail(ctx context.Context, versionID string) (*model.ArtifactVersionDetail, error) {
	if r.Artifacts == nil {
		return nil, errors.New("Artifact service is unavailable")
	}
	stored, version, err := r.Store.ArtifactVersionWithArtifact(ctx, versionID)
	if errors.Is(err, store.ErrArtifactNotFound) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	authorized, err := r.Store.ArtifactOwnerAuthorized(ctx, stored.Artifact.Owner)
	if err != nil || !authorized {
		return nil, err
	}
	title := stored.Artifact.Title
	if version.Title != nil {
		title = *version.Title
	}
	detail := &model.ArtifactVersionDetail{
		ArtifactVersionID: version.ID, ArtifactID: version.ArtifactID,
		VersionIndex: int(version.Index), Title: title, ArtifactKind: stored.Artifact.Kind,
		StorageKind: artifactStorageKind(stored.Artifact.StorageKind), MediaType: version.MediaType,
		Versions: make([]*model.ArtifactVersion, 0, len(stored.Versions)),
	}
	for _, item := range stored.Versions {
		mapped, err := artifactVersionModel(item)
		if err != nil {
			return nil, err
		}
		detail.Versions = append(detail.Versions, mapped)
	}
	if version.ExternalURL != nil {
		detail.PreviewKind = model.ArtifactVersionPreviewKindExternal
		detail.ExternalURL = version.ExternalURL
		return detail, nil
	}
	download := artifact.DownloadURL(version.ID)
	detail.DownloadURL = &download
	if version.MediaType != nil && documents.IsSpreadsheet(*version.MediaType) {
		file, err := r.Artifacts.Read(stored.Artifact, version)
		if err != nil {
			return nil, err
		}
		content, converted := documents.SpreadsheetMarkdown(file.Bytes, *version.MediaType)
		if converted {
			detail.PreviewKind = model.ArtifactVersionPreviewKindMarkdown
			detail.Markdown = &content
		} else {
			detail.PreviewKind = model.ArtifactVersionPreviewKindUnsupported
		}
		return detail, nil
	}
	switch localPreviewKind(version.MediaType) {
	case model.ArtifactVersionPreviewKindImage, model.ArtifactVersionPreviewKindPDF:
		detail.PreviewKind = localPreviewKind(version.MediaType)
		preview := artifact.PreviewURL(version.ID)
		detail.PreviewURL = &preview
	case model.ArtifactVersionPreviewKindMarkdown, model.ArtifactVersionPreviewKindPlainText, model.ArtifactVersionPreviewKindHTML:
		detail.PreviewKind = localPreviewKind(version.MediaType)
		file, err := r.Artifacts.Read(stored.Artifact, version)
		if err != nil {
			return nil, err
		}
		if !utf8.Valid(file.Bytes) {
			return nil, errors.New("Artifact text content is not valid UTF-8")
		}
		content := string(file.Bytes)
		switch detail.PreviewKind {
		case model.ArtifactVersionPreviewKindMarkdown:
			detail.Markdown = &content
		case model.ArtifactVersionPreviewKindPlainText:
			detail.PlainText = &content
		case model.ArtifactVersionPreviewKindHTML:
			detail.HTML = &content
		}
	default:
		detail.PreviewKind = model.ArtifactVersionPreviewKindUnsupported
	}
	return detail, nil
}

func artifactModel(value store.ArtifactWithVersions) (*model.Artifact, error) {
	version, err := artifactVersionModel(value.CurrentVersion)
	if err != nil {
		return nil, err
	}
	return &model.Artifact{
		ArtifactID: value.Artifact.ID, OwnerObjectType: value.Artifact.Owner.ObjectType,
		OwnerObjectID: value.Artifact.Owner.ObjectID, Title: value.Artifact.Title,
		Description: value.Artifact.Description, ArtifactKind: value.Artifact.Kind,
		StorageKind: artifactStorageKind(value.Artifact.StorageKind), CurrentVersion: version,
	}, nil
}

func artifactVersionModel(value store.ArtifactVersion) (*model.ArtifactVersion, error) {
	if value.Index > math.MaxInt32 || value.Index < 1 {
		return nil, errors.New("Artifact version index exceeds GraphQL Int range")
	}
	result := &model.ArtifactVersion{
		ArtifactVersionID: value.ID, ArtifactID: value.ArtifactID, VersionIndex: int(value.Index),
		ExternalURL: value.ExternalURL, MediaType: value.MediaType,
	}
	if value.LocalRelativePath != nil {
		download := artifact.DownloadURL(value.ID)
		result.DownloadURL = &download
	}
	if value.ByteSize != nil {
		if *value.ByteSize > math.MaxInt32 {
			return nil, errors.New("Artifact byte size exceeds GraphQL Int range")
		}
		size := int(*value.ByteSize)
		result.ByteSize = &size
	}
	return result, nil
}

func artifactStorageKind(value string) model.ArtifactStorageKind {
	if value == store.ArtifactExternalURL {
		return model.ArtifactStorageKindExternalURL
	}
	return model.ArtifactStorageKindLocalFile
}

func localPreviewKind(mediaType *string) model.ArtifactVersionPreviewKind {
	if mediaType == nil {
		return model.ArtifactVersionPreviewKindUnsupported
	}
	normalized := strings.ToLower(strings.TrimSpace(strings.SplitN(*mediaType, ";", 2)[0]))
	switch normalized {
	case "text/markdown", "text/x-markdown":
		return model.ArtifactVersionPreviewKindMarkdown
	case "text/plain", "message/rfc822":
		return model.ArtifactVersionPreviewKindPlainText
	case "text/html":
		return model.ArtifactVersionPreviewKindHTML
	case "application/pdf":
		return model.ArtifactVersionPreviewKindPDF
	case "image/png", "image/jpeg", "image/gif", "image/webp", "image/avif":
		return model.ArtifactVersionPreviewKindImage
	case "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
		"application/vnd.ms-excel", "application/vnd.oasis.opendocument.spreadsheet":
		return model.ArtifactVersionPreviewKindUnsupported
	default:
		return model.ArtifactVersionPreviewKindUnsupported
	}
}

func validArtifactLabel(value string, max int) bool {
	count := utf8.RuneCountInString(value)
	return count > 0 && count <= max && strings.TrimSpace(value) != ""
}
