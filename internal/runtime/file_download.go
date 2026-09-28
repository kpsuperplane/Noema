package runtime

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/netpolicy"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	fileDownloadName       = "file.download"
	fileDownloadLimit      = 32 << 20
	fileDownloadTimeout    = 30 * time.Second
	fileDownloadRedirects  = 3
	fileDownloadMaximumURL = 2048
	fileDownloadReason     = 500
)

var fileDownloadSchema = json.RawMessage(`{
  "type":"object",
  "properties":{
    "url":{"type":"string","minLength":1,"maxLength":2048},
    "path":{"type":"string","minLength":1,"maxLength":4096},
    "parse":{"type":"boolean","default":false},
    "max_chars":{"type":"integer","minimum":1000,"maximum":20000},
    "reason":{"type":"string","minLength":1,"maxLength":500}
  },
  "required":["url","path"],
  "additionalProperties":false
}`)

var errDownloadOutcomeUncertain = errors.New("download outcome is uncertain")

type fileDownloadRequest struct {
	URL, Path string
	Parse     bool
	MaxChars  int
	Reason    string
}

func fileDownloadTool() provider.GenerationTool {
	return provider.GenerationTool{
		Name:        fileDownloadName,
		Description: "Download a public non-HTML resource into the current working directory. The destination must not exist.",
		InputSchema: append(json.RawMessage(nil), fileDownloadSchema...),
	}
}

func parseFileDownloadArguments(raw json.RawMessage) (fileDownloadRequest, error) {
	fields, err := uniqueDownloadArguments(raw)
	if err != nil || len(fields) < 2 || len(fields) > 5 {
		return fileDownloadRequest{}, errors.New("arguments do not match the file.download schema")
	}
	for name := range fields {
		if name != "url" && name != "path" && name != "parse" && name != "max_chars" && name != "reason" {
			return fileDownloadRequest{}, errors.New("arguments do not match the file.download schema")
		}
	}
	request := fileDownloadRequest{MaxChars: fileParseMaximumCharacters}
	if json.Unmarshal(fields["url"], &request.URL) != nil || json.Unmarshal(fields["path"], &request.Path) != nil {
		return fileDownloadRequest{}, errors.New("arguments do not match the file.download schema")
	}
	request.URL, request.Path = strings.TrimSpace(request.URL), strings.TrimSpace(request.Path)
	if request.URL == "" || utf8.RuneCountInString(request.URL) > fileDownloadMaximumURL ||
		request.Path == "" || utf8.RuneCountInString(request.Path) > fileParseMaximumPath {
		return fileDownloadRequest{}, errors.New("url or path is missing or too long")
	}
	parsedURL, err := url.Parse(request.URL)
	if err != nil {
		return fileDownloadRequest{}, errors.New("url is invalid")
	}
	if parsedURL.User != nil {
		return fileDownloadRequest{}, errors.New("credential-bearing URLs are unavailable")
	}
	if value, ok := fields["parse"]; ok && json.Unmarshal(value, &request.Parse) != nil {
		return fileDownloadRequest{}, errors.New("arguments do not match the file.download schema")
	}
	if value, ok := fields["max_chars"]; ok && (json.Unmarshal(value, &request.MaxChars) != nil ||
		request.MaxChars < fileParseMinimumCharacters || request.MaxChars > fileParseMaximumCharacters) {
		return fileDownloadRequest{}, errors.New("arguments do not match the file.download schema")
	}
	if value, ok := fields["reason"]; ok {
		if json.Unmarshal(value, &request.Reason) != nil {
			return fileDownloadRequest{}, errors.New("arguments do not match the file.download schema")
		}
		request.Reason = strings.TrimSpace(request.Reason)
		if request.Reason == "" || utf8.RuneCountInString(request.Reason) > fileDownloadReason {
			return fileDownloadRequest{}, errors.New("reason is invalid")
		}
	}
	return request, nil
}

func uniqueDownloadArguments(raw json.RawMessage) (map[string]json.RawMessage, error) {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	token, err := decoder.Token()
	if err != nil || token != json.Delim('{') {
		return nil, errors.New("arguments are invalid")
	}
	fields := make(map[string]json.RawMessage)
	for decoder.More() {
		nameToken, err := decoder.Token()
		name, ok := nameToken.(string)
		if err != nil || !ok {
			return nil, errors.New("arguments are invalid")
		}
		if _, exists := fields[name]; exists {
			return nil, errors.New("arguments are invalid")
		}
		var value json.RawMessage
		if err := decoder.Decode(&value); err != nil {
			return nil, errors.New("arguments are invalid")
		}
		fields[name] = value
	}
	if token, err = decoder.Token(); err != nil || token != json.Delim('}') {
		return nil, errors.New("arguments are invalid")
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		return nil, errors.New("arguments are invalid")
	}
	return fields, nil
}

// Observed URLs use the same download validation and file-write path as reviewed URLs.
func executeObservedDownload(ctx context.Context, database *store.Store, cwd string, raw json.RawMessage) (json.RawMessage, bool, bool, error) {
	request, err := parseFileDownloadArguments(raw)
	if err != nil {
		return nil, false, false, err
	}
	observed, err := database.URLWasObserved(ctx, request.URL)
	if err != nil || !observed {
		return nil, false, false, err
	}
	payload, err := executeFileDownload(ctx, cwd, raw)
	if err != nil {
		code := "download_failed"
		if errors.Is(err, errDownloadOutcomeUncertain) {
			code = "outcome_uncertain"
		}
		return toolFailure(code, err.Error()), false, true, nil
	}
	return payload, true, true, nil
}

func executeFileDownload(ctx context.Context, cwd string, raw json.RawMessage) (json.RawMessage, error) {
	request, err := parseFileDownloadArguments(raw)
	if err != nil {
		return nil, err
	}
	if cwd == "" {
		return nil, errors.New("conversation working directory is unavailable")
	}
	relative, err := normalizedFilePath(request.Path)
	if err != nil {
		return nil, err
	}
	root, err := os.OpenRoot(cwd)
	if err != nil {
		return nil, errors.New("working directory is unavailable")
	}
	defer root.Close()
	if err := prepareDownloadParent(root, relative); err != nil {
		return nil, err
	}
	if _, err := root.Lstat(relative); err == nil {
		return nil, errors.New("destination already exists")
	} else if !errors.Is(err, os.ErrNotExist) {
		return nil, errors.New("destination is unavailable")
	}
	temporary, err := downloadTemporaryPath(relative)
	if err != nil {
		return nil, errors.New("temporary download file is unavailable")
	}
	file, err := root.OpenFile(temporary, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if err != nil {
		return nil, errors.New("temporary download file could not be created")
	}
	defer func() { _ = root.Remove(temporary) }()
	downloadContext, cancel := context.WithTimeout(ctx, fileDownloadTimeout)
	defer cancel()
	finalURL, count, mediaType, err := downloadPublicFile(downloadContext, file, request.URL)
	if err != nil {
		_ = file.Close()
		return nil, err
	}
	if err := file.Sync(); err != nil {
		_ = file.Close()
		return nil, errors.New("download write failed")
	}
	if err := file.Close(); err != nil {
		return nil, errors.New("download write failed")
	}
	if err := root.Link(temporary, relative); err != nil {
		if _, existsErr := root.Lstat(relative); existsErr == nil {
			return nil, errors.New("destination already exists")
		}
		return nil, errors.New("download could not be committed")
	}
	if err := root.Remove(temporary); err != nil {
		return nil, errDownloadOutcomeUncertain
	}
	parent := filepath.Dir(relative)
	if err := home.SyncRootDirectory(root, parent); err != nil {
		return nil, errDownloadOutcomeUncertain
	}
	var parsed any
	if request.Parse {
		arguments, _ := json.Marshal(map[string]any{"path": request.Path, "max_chars": request.MaxChars})
		responseMediaType, _ := mediaType.(string)
		result, parseErr := parseConversationFileWithMedia(ctx, cwd, arguments, responseMediaType)
		if parseErr != nil {
			parsed = map[string]string{"error": parseErr.Error()}
		} else {
			parsed = result
		}
	}
	payload, _ := json.Marshal(map[string]any{
		"url": request.URL, "final_url": finalURL, "path": request.Path,
		"saved_bytes": count, "media_type": mediaType, "parse": parsed,
	})
	return payload, nil
}

func downloadPublicFile(ctx context.Context, destination *os.File, rawURL string) (string, int64, any, error) {
	checked, err := netpolicy.CheckURL(ctx, rawURL)
	if err != nil {
		return "", 0, nil, errors.New("download URL is unavailable")
	}
	for redirect := 0; ; redirect++ {
		client := netpolicy.PinnedClient(checked, fileDownloadTimeout)
		request, _ := http.NewRequestWithContext(ctx, http.MethodGet, checked.URL.String(), nil)
		request.Header.Set("User-Agent", "NoemaFileDownload/0.1 (+https://github.com/kpsuperplane/Noema)")
		response, requestErr := client.Do(request)
		client.CloseIdleConnections()
		if requestErr != nil {
			return "", 0, nil, errors.New("download request failed")
		}
		if response.StatusCode >= 300 && response.StatusCode < 400 {
			_ = response.Body.Close()
			if redirect >= fileDownloadRedirects {
				return "", 0, nil, errors.New("download has too many redirects")
			}
			location := response.Header.Get("Location")
			next, joinErr := checked.URL.Parse(location)
			if joinErr != nil || location == "" {
				return "", 0, nil, errors.New("download redirect is invalid")
			}
			checked, err = netpolicy.CheckURL(ctx, next.String())
			if err != nil {
				return "", 0, nil, errors.New("download redirect is unavailable")
			}
			continue
		}
		count, mediaValue, err := saveDownloadResponse(destination, response)
		if err != nil {
			return "", 0, nil, err
		}
		return checked.URL.String(), count, mediaValue, nil
	}
}

func saveDownloadResponse(destination io.Writer, response *http.Response) (int64, any, error) {
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return 0, nil, errors.New("download request failed")
	}
	if response.ContentLength > fileDownloadLimit {
		return 0, nil, errors.New("download exceeds the size limit")
	}
	media := strings.ToLower(strings.TrimSpace(strings.Split(response.Header.Get("Content-Type"), ";")[0]))
	if media == "text/html" || media == "application/xhtml+xml" {
		return 0, nil, errors.New("HTML responses cannot be downloaded with file.download")
	}
	count, err := io.Copy(destination, io.LimitReader(response.Body, fileDownloadLimit+1))
	if err != nil {
		return 0, nil, errors.New("download request failed")
	}
	if count > fileDownloadLimit {
		return 0, nil, errors.New("download exceeds the size limit")
	}
	if media == "" {
		return count, nil, nil
	}
	return count, media, nil
}

func prepareDownloadParent(root *os.Root, relative string) error {
	parent := filepath.Dir(relative)
	if parent == "." {
		return nil
	}
	current := ""
	for _, part := range strings.Split(filepath.ToSlash(parent), "/") {
		current = filepath.Join(current, part)
		metadata, err := root.Lstat(current)
		switch {
		case err == nil && metadata.Mode()&os.ModeSymlink != 0:
			return errors.New("file path contains a symbolic link")
		case err == nil && !metadata.IsDir():
			return errors.New("download parent is not a directory")
		case err == nil:
		case errors.Is(err, os.ErrNotExist):
			if err := root.Mkdir(current, 0o700); err != nil {
				return errors.New("download directory could not be created")
			}
		default:
			return errors.New("download directory is unavailable")
		}
	}
	return nil
}

func downloadTemporaryPath(relative string) (string, error) {
	var random [8]byte
	if _, err := rand.Read(random[:]); err != nil {
		return "", err
	}
	name := filepath.Base(relative)
	return filepath.Join(filepath.Dir(relative), fmt.Sprintf(".%s.noema-%s.tmp", name, hex.EncodeToString(random[:]))), nil
}
