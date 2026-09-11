package adapter

import (
	"context"
	"embed"
	"encoding/json"
	"errors"
	"sync"
)

//go:embed library/*.json
var libraryFiles embed.FS

// LibraryEntry describes release-owned definitions, without account authority.
type LibraryEntry struct {
	ID              string   `json:"id"`
	DefinitionID    string   `json:"definition_id"`
	Name            string   `json:"name"`
	Description     string   `json:"description"`
	Revision        string   `json:"revision"`
	SemanticDigest  string   `json:"semantic_digest"`
	SourceReference string   `json:"source_reference"`
	OperationIDs    []string `json:"operation_ids"`
}

type libraryDefinition struct {
	LibraryEntry
	definition Definition
}

var bundledLibrary = sync.OnceValues(func() ([]libraryDefinition, error) {
	entries := []libraryDefinition{
		{LibraryEntry: LibraryEntry{ID: "gmail", Name: "Gmail", Description: "Read, send, and manage mail.", SourceReference: "https://developers.google.com/workspace/gmail/api/reference/rest"}},
		{LibraryEntry: LibraryEntry{ID: "google-calendar", Name: "Google Calendar", Description: "Manage events and availability.", SourceReference: "https://developers.google.com/workspace/calendar/api/v3/reference"}},
	}
	for i := range entries {
		entry := &entries[i]
		raw, err := libraryFiles.ReadFile("library/" + entry.ID + ".json")
		if err != nil {
			return nil, err
		}
		// Bundled manifests bind to the release's reviewed Google profile.
		var manifest Manifest
		if err = decodeExactJSON(raw, &manifest); err != nil {
			return nil, err
		}
		if manifest.Authentication.ProfileDigest != "google" || !manifest.Reviewed {
			return nil, errors.New("bundled API definition is invalid")
		}
		manifest.Authentication.ProfileDigest = googleOAuthProfile().ProfileDigest
		raw, err = json.Marshal(manifest)
		if err != nil {
			return nil, err
		}
		entry.definition, err = CompileJSON(raw)
		if err != nil {
			return nil, err
		}
		entry.DefinitionID = manifest.DefinitionID
		entry.Revision = manifest.DefinitionRevision
		entry.SemanticDigest = entry.definition.SemanticDigest
		for _, operation := range entry.definition.Operations {
			entry.OperationIDs = append(entry.OperationIDs, operation.OperationID)
		}
	}
	return entries, nil
})

func Library() ([]LibraryEntry, error) {
	definitions, err := bundledLibrary()
	if err != nil {
		return nil, err
	}
	entries := make([]LibraryEntry, len(definitions))
	for i, definition := range definitions {
		entries[i] = definition.LibraryEntry
		entries[i].OperationIDs = append([]string(nil), definition.OperationIDs...)
	}
	return entries, nil
}

func (s *Service) ConnectLibrary(ctx context.Context, id, expectedDigest string) (Definition, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.connectLibrary(ctx, id, expectedDigest)
}

func (s *Service) connectLibrary(ctx context.Context, id, expectedDigest string) (Definition, error) {
	entries, err := bundledLibrary()
	if err != nil {
		return Definition{}, err
	}
	var selected *libraryDefinition
	for i := range entries {
		if entries[i].ID == id && entries[i].SemanticDigest == expectedDigest {
			selected = &entries[i]
		}
	}
	if selected == nil {
		return Definition{}, errors.New("API library selection changed; reload the library")
	}
	definitions, err := s.compiledDefinitions()
	if err != nil {
		return Definition{}, err
	}
	for _, definition := range definitions {
		if definition.Manifest.DefinitionID == selected.definition.Manifest.DefinitionID && !definition.Superseded {
			// Preserve the installed revision, including local pending edits.
			return definition, nil
		}
	}
	s.registry = nil
	definition, err := s.files.installDefinition(selected.definition.Manifest, selected.SourceReference, nil, nil)
	if err != nil {
		return Definition{}, err
	}
	return definition, s.reconcile(ctx)
}

func (s *Service) connectLibraryTool(raw json.RawMessage) (any, error) {
	var input struct {
		LibraryID      string `json:"library_id"`
		ExpectedDigest string `json:"expected_digest"`
	}
	if err := decodeExactJSON(raw, &input); err != nil {
		return nil, errors.New("API library selection is invalid")
	}
	definition, err := s.connectLibrary(context.Background(), input.LibraryID, input.ExpectedDigest)
	if err != nil {
		return nil, err
	}
	return map[string]any{"status": definition.ReviewStatus(), "semantic_digest": definition.SemanticDigest,
		"definition_id": definition.Manifest.DefinitionID, "next_step": "Continue through the account setup and connection permissions shown to the human. Selection does not grant access. Do not research or propose a duplicate definition."}, nil
}
