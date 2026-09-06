package artifact

import (
	"errors"
	"path/filepath"
	"strings"
	"unicode"

	"github.com/kpsuperplane/noema/internal/store"
)

// SafeFilename validates one portable filename component.
func SafeFilename(value string) error {
	if value == "" || strings.HasSuffix(value, ".") || strings.HasSuffix(value, " ") ||
		strings.ContainsAny(value, `\":<>|?*`) {
		return errors.New("unsafe Artifact filename")
	}
	for _, character := range value {
		if unicode.IsControl(character) {
			return errors.New("unsafe Artifact filename")
		}
	}
	if filepath.Base(value) != value || value == "." || value == ".." || windowsDeviceName(value) {
		return errors.New("unsafe Artifact filename")
	}
	return nil
}

// DownloadURL returns the authorized download route for one version.
func DownloadURL(versionID string) string {
	return "/artifacts/versions/" + strings.TrimPrefix(versionID, "artifact_version:") + "/download"
}

// PreviewURL returns the authorized inline route for one version.
func PreviewURL(versionID string) string {
	return "/artifacts/versions/" + strings.TrimPrefix(versionID, "artifact_version:") + "/preview"
}

// VersionIDFromSlug converts one public route component to an identifier.
func VersionIDFromSlug(slug string) (string, bool) {
	if slug == "" || strings.ContainsAny(slug, "/:") {
		return "", false
	}
	return "artifact_version:" + slug, true
}

func versionDirectory(owner store.ArtifactOwner, artifactID string, index int64) string {
	base := "tasks"
	if owner.ObjectType == "conversation" {
		base = "conversations"
	}
	return filepath.Join(base, sanitizeSegment(owner.ObjectID), "artifacts", sanitizeSegment(artifactID), "versions", formatIndex(index))
}

func validateStoredPath(artifact store.Artifact, version store.ArtifactVersion) (string, string, error) {
	if version.LocalRelativePath == nil || version.Index < 1 {
		return "", "", ErrUnavailable
	}
	path := filepath.FromSlash(*version.LocalRelativePath)
	if filepath.IsAbs(path) || filepath.Clean(path) != path {
		return "", "", ErrUnavailable
	}
	parent, filename := filepath.Dir(path), filepath.Base(path)
	if SafeFilename(filename) != nil {
		return "", "", ErrUnavailable
	}
	expectedPrefix := filepath.Join(versionDirectory(artifact.Owner, artifact.ID, version.Index), "objects")
	opDirectory := filepath.Dir(parent)
	if opDirectory != expectedPrefix || !validOperationID(filepath.Base(parent)) {
		return "", "", ErrUnavailable
	}
	return filename, parent, nil
}

func sanitizeSegment(value string) string {
	var result strings.Builder
	for _, character := range value {
		if character >= 'a' && character <= 'z' || character >= 'A' && character <= 'Z' ||
			character >= '0' && character <= '9' || character == '-' || character == '_' {
			result.WriteRune(character)
		} else {
			result.WriteByte('_')
		}
	}
	if result.Len() == 0 {
		return "_"
	}
	return result.String()
}

func formatIndex(value int64) string {
	const digits = "0123456789"
	if value <= 0 {
		return "0"
	}
	var buffer [20]byte
	position := len(buffer)
	for value > 0 {
		position--
		buffer[position] = digits[value%10]
		value /= 10
	}
	return string(buffer[position:])
}

func windowsDeviceName(value string) bool {
	base := strings.ToUpper(strings.SplitN(value, ".", 2)[0])
	if base == "CON" || base == "PRN" || base == "AUX" || base == "NUL" {
		return true
	}
	if len(base) == 4 && (strings.HasPrefix(base, "COM") || strings.HasPrefix(base, "LPT")) {
		return base[3] >= '1' && base[3] <= '9'
	}
	return false
}
