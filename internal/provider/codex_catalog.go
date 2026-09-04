package provider

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/url"
	"strings"
	"time"
)

const (
	codexFallbackClientVersion  = "0.144.0"
	codexCatalogMetadataVersion = 3
	codexCatalogUserAgent       = "Noema/0.1 (+https://github.com/kpsuperplane/Noema)"
)

type codexModelCatalog struct {
	Profiles         []ModelProfile
	ClientVersion    string
	VersionFetchedAt *time.Time
}

func (s *CodexService) fetchModelCatalog(
	ctx context.Context,
	tokens CodexTokens,
) (codexModelCatalog, error) {
	version, fetchedAt := s.fetchClientVersion(ctx)
	endpoint := strings.TrimRight(s.modelsBaseURL, "/") + "/models?client_version=" +
		url.QueryEscape(version)
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return codexModelCatalog{}, codexRemoteError{kind: codexUnavailable}
	}
	request.Header.Set("Accept", "application/json")
	var response *http.Response
	err = tokens.Use(func(accessToken string, _ string, _ uint64) error {
		request.Header.Set("Authorization", "Bearer "+accessToken)
		var sendErr error
		response, sendErr = s.client.Do(request)
		request.Header.Del("Authorization")
		if response != nil && response.Request != nil {
			response.Request.Header.Del("Authorization")
		}
		return sendErr
	})
	if err != nil || response == nil {
		return codexModelCatalog{}, codexRemoteError{kind: codexNetwork}
	}
	defer response.Body.Close()
	data, err := readCodexResponse(response.Body)
	if err != nil {
		return codexModelCatalog{}, codexRemoteError{kind: codexMalformed}
	}
	if response.StatusCode == http.StatusUnauthorized || response.StatusCode == http.StatusForbidden {
		return codexModelCatalog{}, codexRemoteError{kind: codexRejected}
	}
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		return codexModelCatalog{}, codexRemoteError{kind: codexUnavailable}
	}
	profiles, err := codexProfiles(data)
	if err != nil || len(profiles) == 0 {
		return codexModelCatalog{}, codexRemoteError{kind: codexMalformed}
	}
	return codexModelCatalog{
		Profiles: profiles, ClientVersion: version, VersionFetchedAt: fetchedAt,
	}, nil
}

func (s *CodexService) fetchClientVersion(ctx context.Context) (string, *time.Time) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, s.versionURL, nil)
	if err != nil {
		return codexFallbackClientVersion, nil
	}
	request.Header.Set("Accept", "application/json")
	request.Header.Set("User-Agent", codexCatalogUserAgent)
	response, err := s.client.Do(request)
	if err != nil || response == nil {
		return codexFallbackClientVersion, nil
	}
	defer response.Body.Close()
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		return codexFallbackClientVersion, nil
	}
	data, err := readCodexResponse(response.Body)
	if err != nil {
		return codexFallbackClientVersion, nil
	}
	var payload struct {
		Version string `json:"version"`
	}
	if decodeCodexRemoteJSON(data, &payload) != nil || !validCodexClientVersion(payload.Version) {
		return codexFallbackClientVersion, nil
	}
	now := s.now().UTC()
	return strings.TrimSpace(payload.Version), &now
}

func validCodexClientVersion(value string) bool {
	value = strings.TrimSpace(value)
	core, suffix, prerelease := value, "", false
	if index := strings.IndexByte(value, '-'); index >= 0 {
		core, suffix, prerelease = value[:index], value[index+1:], true
	}
	parts := strings.Split(core, ".")
	if len(parts) != 3 {
		return false
	}
	for _, part := range parts {
		if part == "" {
			return false
		}
		for _, character := range part {
			if character < '0' || character > '9' {
				return false
			}
		}
	}
	if !prerelease {
		return true
	}
	if suffix == "" {
		return false
	}
	for _, character := range suffix {
		if !((character >= 'a' && character <= 'z') ||
			(character >= 'A' && character <= 'Z') ||
			(character >= '0' && character <= '9') || character == '.' || character == '-') {
			return false
		}
	}
	return true
}

func codexProfiles(data []byte) ([]ModelProfile, error) {
	var payload struct {
		Models []json.RawMessage `json:"models"`
	}
	if err := decodeCodexRemoteJSON(data, &payload); err != nil {
		return nil, errors.New("Codex model catalog is invalid")
	}
	profiles := make([]ModelProfile, 0, len(payload.Models))
	seen := make(map[string]struct{}, len(payload.Models))
	for _, raw := range payload.Models {
		profile, visible := codexProfile(raw)
		if !visible {
			continue
		}
		if _, exists := seen[profile.ID]; exists {
			continue
		}
		seen[profile.ID] = struct{}{}
		profiles = append(profiles, profile)
	}
	return profiles, nil
}

func codexProfile(raw json.RawMessage) (ModelProfile, bool) {
	var fields map[string]json.RawMessage
	if json.Unmarshal(raw, &fields) != nil {
		return ModelProfile{}, false
	}
	visibility, visible := rawString(fields["visibility"])
	id, idOK := rawString(fields["slug"])
	id = strings.TrimSpace(id)
	if !visible || visibility != "list" || !idOK || id == "" {
		return ModelProfile{}, false
	}
	label, _ := rawString(fields["display_name"])
	label = strings.TrimSpace(label)
	if label == "" {
		label = id
	}
	profile := ModelProfile{ID: id, Label: label}
	for _, key := range []string{"supported_reasoning_levels", "reasoning_levels", "reasoning_efforts"} {
		values, ok := rawStrings(fields[key])
		if !ok {
			continue
		}
		for _, value := range values {
			if effort := normalizeReasoningEffort(value); effort != "" {
				profile.ReasoningEfforts = append(profile.ReasoningEfforts, effort)
			}
		}
		if len(profile.ReasoningEfforts) != 0 {
			break
		}
	}
	for _, key := range []string{"default_reasoning_level", "default_reasoning_effort"} {
		if value, ok := rawString(fields[key]); ok {
			profile.DefaultReasoningEffort = normalizeReasoningEffort(value)
			if profile.DefaultReasoningEffort != "" {
				break
			}
		}
	}
	if len(profile.ReasoningEfforts) == 0 && profile.DefaultReasoningEffort != "" {
		profile.ReasoningEfforts = []string{"low", "medium", "high", "xhigh"}
	}
	return profile, true
}

func codexMetadataWithCatalog(
	source AccountMetadata,
	catalog codexModelCatalog,
	now time.Time,
) (AccountMetadata, error) {
	metadata, err := metadataWithProfiles(source, catalog.Profiles, now)
	if err != nil {
		return nil, err
	}
	values := map[string]any{
		"models_source":           "codex_models_endpoint",
		"models_metadata_version": codexCatalogMetadataVersion,
		"models_client_version":   catalog.ClientVersion,
	}
	if catalog.VersionFetchedAt != nil {
		values["models_client_version_refreshed_at"] = catalog.VersionFetchedAt.UTC().Unix()
	} else {
		delete(metadata, "models_client_version_refreshed_at")
	}
	for key, value := range values {
		encoded, err := json.Marshal(value)
		if err != nil {
			return nil, errors.New("encode Codex model metadata")
		}
		metadata[key] = encoded
	}
	return metadata, nil
}
