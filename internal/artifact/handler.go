package artifact

import (
	"fmt"
	"mime"
	"net/http"
	"strconv"
	"strings"
)

// Handler serves authorized local Artifact downloads and previews.
func (s *Service) Handler() http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet && r.Method != http.MethodHead {
			http.NotFound(w, r)
			return
		}
		tail, ok := strings.CutPrefix(r.URL.Path, "/artifacts/versions/")
		if !ok {
			http.NotFound(w, r)
			return
		}
		parts := strings.Split(tail, "/")
		if len(parts) != 2 || parts[1] != "download" && parts[1] != "preview" {
			http.NotFound(w, r)
			return
		}
		versionID, ok := VersionIDFromSlug(parts[0])
		if !ok {
			http.NotFound(w, r)
			return
		}
		file, found, err := s.AuthorizedFile(r.Context(), versionID)
		if err != nil {
			http.Error(w, "internal server error", http.StatusInternalServerError)
			return
		}
		if !found {
			http.NotFound(w, r)
			return
		}
		if parts[1] == "preview" {
			mediaType := normalizedMediaType(file.MediaType)
			if !inlineMediaType(mediaType) {
				http.Error(w, "preview unavailable", http.StatusUnsupportedMediaType)
				return
			}
			w.Header().Set("Content-Type", mediaType)
			w.Header().Set("Content-Disposition", contentDisposition("inline", file.Filename))
			w.Header().Set("Content-Security-Policy", "default-src 'none'; frame-ancestors 'self'; base-uri 'none'; form-action 'none'")
		} else {
			w.Header().Set("Content-Type", safeMediaType(file.MediaType))
			w.Header().Set("Content-Disposition", contentDisposition("attachment", file.Filename))
		}
		w.Header().Set("Content-Length", strconv.Itoa(len(file.Bytes)))
		if r.Method == http.MethodGet {
			_, _ = w.Write(file.Bytes)
		}
	})
}

func normalizedMediaType(value string) string {
	mediaType, _, err := mime.ParseMediaType(value)
	if err != nil {
		return ""
	}
	return strings.ToLower(mediaType)
}

func safeMediaType(value string) string {
	if normalizedMediaType(value) == "" {
		return "application/octet-stream"
	}
	return value
}

func inlineMediaType(value string) bool {
	switch value {
	case "application/pdf", "image/png", "image/jpeg", "image/gif", "image/webp", "image/avif":
		return true
	default:
		return false
	}
}

func contentDisposition(kind, filename string) string {
	value := mime.FormatMediaType(kind, map[string]string{"filename": filename})
	if value == "" {
		return fmt.Sprintf("%s; filename=%q", kind, "artifact")
	}
	return value
}
