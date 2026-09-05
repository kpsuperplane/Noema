package documents

import (
	"archive/zip"
	"bytes"
	"errors"
	"io"
	"path"
	"strings"
	"unicode/utf8"
)

const (
	MediaDOCX = "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
	MediaDOC  = "application/msword"
	MediaODT  = "application/vnd.oasis.opendocument.text"
	MediaPPTX = "application/vnd.openxmlformats-officedocument.presentationml.presentation"
	MediaPPT  = "application/vnd.ms-powerpoint"
	MediaODP  = "application/vnd.oasis.opendocument.presentation"
	MediaRTF  = "application/rtf"
	MediaPDF  = "application/pdf"
	MediaEPUB = "application/epub+zip"

	maxDocumentInputBytes    = 32 * 1024 * 1024
	maxDocumentEntries       = 10_000
	maxDocumentPartBytes     = 32 * 1024 * 1024
	maxDocumentExpandedBytes = 64 * 1024 * 1024
	maxDocumentXMLDepth      = 256
	maxDocumentXMLTokens     = 2_000_000
	maxDocumentBlocks        = 4_096
	maxDocumentTextBytes     = maxPreviewCharacters * utf8.UTFMax
)

var errInvalidDocument = errors.New("invalid document")

// IsDocument reports whether a media type has a bounded Markdown parser.
func IsDocument(mediaType string) bool {
	switch normalizedMediaType(mediaType) {
	case MediaXLSX, MediaXLS, MediaODS, MediaDOCX, MediaDOC, MediaODT, MediaPPTX, MediaPPT, MediaODP,
		MediaRTF, "text/rtf", "application/x-rtf", MediaPDF, MediaEPUB:
		return true
	default:
		return false
	}
}

// DocumentMarkdown converts one supported document into bounded Markdown.
// Malformed, encrypted, and resource-heavy inputs produce no parser diagnostics.
func DocumentMarkdown(data []byte, mediaType string) (content string, converted bool) {
	mediaType = normalizedMediaType(mediaType)
	if mediaType == MediaXLSX || mediaType == MediaXLS || mediaType == MediaODS {
		return SpreadsheetMarkdown(data, mediaType)
	}
	if len(data) == 0 || len(data) > maxDocumentInputBytes {
		return "", false
	}
	defer func() {
		if recover() != nil {
			content, converted = "", false
		}
	}()

	var result string
	var err error
	switch mediaType {
	case MediaDOC:
		result, err = parseDOC(data)
	case MediaDOCX:
		result, err = parseDOCX(data)
	case MediaODT:
		result, err = parseODF(data, false)
	case MediaPPTX:
		result, err = parsePPTX(data)
	case MediaPPT:
		result, err = parsePPT(data)
	case MediaODP:
		result, err = parseODF(data, true)
	case MediaRTF, "text/rtf", "application/x-rtf":
		result, err = parseRTF(data)
	case MediaPDF:
		result, err = parsePDF(data)
	case MediaEPUB:
		result, err = parseEPUB(data)
	default:
		return "", false
	}
	if err != nil {
		return "", false
	}
	return result, true
}

type documentArchive struct {
	entries map[string]*zip.File
	read    uint64
}

func openDocumentArchive(data []byte) (*documentArchive, error) {
	archive, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil || len(archive.File) > maxDocumentEntries {
		return nil, errInvalidDocument
	}
	entries := make(map[string]*zip.File, len(archive.File))
	for _, file := range archive.File {
		name := path.Clean(strings.TrimPrefix(file.Name, "/"))
		if name == "." || name == ".." || strings.HasPrefix(name, "../") {
			return nil, errInvalidDocument
		}
		if _, duplicate := entries[name]; duplicate {
			return nil, errInvalidDocument
		}
		entries[name] = file
	}
	return &documentArchive{entries: entries}, nil
}

func (archive *documentArchive) part(name string, required bool) ([]byte, error) {
	file, ok := archive.entries[strings.TrimPrefix(name, "/")]
	if !ok {
		if required {
			return nil, errInvalidDocument
		}
		return nil, nil
	}
	if file.UncompressedSize64 > maxDocumentPartBytes ||
		archive.read > maxDocumentExpandedBytes-file.UncompressedSize64 {
		return nil, errInvalidDocument
	}
	reader, err := file.Open()
	if err != nil {
		return nil, errInvalidDocument
	}
	defer reader.Close()
	content, err := io.ReadAll(io.LimitReader(reader, maxDocumentPartBytes+1))
	if err != nil || len(content) > maxDocumentPartBytes {
		return nil, errInvalidDocument
	}
	archive.read += uint64(len(content))
	if archive.read > maxDocumentExpandedBytes {
		return nil, errInvalidDocument
	}
	return content, nil
}

func resolveDocumentPart(base, target string) (string, bool) {
	if strings.Contains(target, "\\") || strings.Contains(target, "?") || strings.Contains(target, "#") {
		return "", false
	}
	if strings.HasPrefix(target, "/") {
		target = strings.TrimPrefix(target, "/")
	} else {
		target = path.Join(path.Dir(base), target)
	}
	target = path.Clean(target)
	return target, target != "." && target != ".." && !strings.HasPrefix(target, "../")
}

type markdownDocument struct {
	text   strings.Builder
	count  int
	blocks int
}

func appendDocumentText(builder *strings.Builder, value string) {
	for _, character := range value {
		if builder.Len()+utf8.RuneLen(character) > maxDocumentTextBytes {
			return
		}
		builder.WriteRune(character)
	}
}

func appendDocumentRune(builder *strings.Builder, value rune) {
	if builder.Len()+utf8.RuneLen(value) <= maxDocumentTextBytes {
		builder.WriteRune(value)
	}
}

func appendDocumentSpaces(builder *strings.Builder, count int) {
	count = min(count, maxDocumentTextBytes-builder.Len())
	if count > 0 {
		builder.WriteString(strings.Repeat(" ", count))
	}
}

func (document *markdownDocument) block(prefix, value string) {
	if document.count >= maxPreviewCharacters || document.blocks >= maxDocumentBlocks {
		return
	}
	value = strings.TrimSpace(strings.ReplaceAll(strings.ReplaceAll(value, "\r\n", "\n"), "\r", "\n"))
	if value == "" {
		return
	}
	if document.blocks > 0 {
		document.append("\n\n")
	}
	document.append(prefix)
	lines := strings.Split(value, "\n")
	for index, line := range lines {
		if index > 0 {
			document.append("  \n")
			if strings.HasPrefix(prefix, "> ") {
				document.append("> ")
			}
		}
		document.append(escapeMarkdown(strings.TrimSpace(line), false))
	}
	document.blocks++
}

func (document *markdownDocument) append(value string) {
	remaining := maxPreviewCharacters - document.count
	for _, character := range value {
		if remaining == 0 {
			return
		}
		document.text.WriteRune(character)
		document.count++
		remaining--
	}
}

func (document *markdownDocument) String() string {
	result := strings.TrimRight(document.text.String(), "\n")
	if !utf8.ValidString(result) {
		return ""
	}
	return result
}
