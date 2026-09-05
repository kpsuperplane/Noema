package runtime

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/abemedia/go-cfb"
	"github.com/kpsuperplane/noema/internal/documents"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	fileParseName              = "file.parse"
	fileParseMaximumPath       = 4_096
	fileParseMaximumInput      = 32 << 20
	fileParseMaximumCharacters = 20_000
	fileParseMinimumCharacters = 1_000
	fileParseTimeout           = 30 * time.Second
	fileParseWorkerMemory      = 512 << 20
	fileParseWorkerEnvironment = "NOEMA_FILE_PARSE_WORKER"
	fileParseMediaEnvironment  = "NOEMA_FILE_PARSE_MEDIA"
)

var (
	fileParseSchema = json.RawMessage(`{
  "type":"object",
  "properties":{
    "path":{"type":"string","minLength":1,"maxLength":4096,"description":"Path relative to the current working directory."},
    "max_chars":{"type":"integer","minimum":1000,"maximum":20000,"description":"Maximum parsed characters to return."}
  },
  "required":["path"],
  "additionalProperties":false
}`)
	documentParsePermit = make(chan struct{}, 1)
)

type fileParseResponse struct {
	Path          string  `json:"path"`
	SourceBytes   int64   `json:"source_bytes"`
	Status        string  `json:"status"`
	Parser        *string `json:"parser"`
	Format        *string `json:"format"`
	ContentFormat *string `json:"content_format"`
	Content       *string `json:"content"`
	ReturnedChars int     `json:"returned_chars"`
	Truncated     bool    `json:"truncated"`
	Error         *string `json:"error"`
}

type fileParseRequest struct {
	path     string
	maxChars int
}

type documentConversion struct {
	Content   string `json:"content"`
	Converted bool   `json:"converted"`
	ErrorCode string `json:"error,omitempty"`
}

func fileParseTool() provider.GenerationTool {
	return provider.GenerationTool{
		Name:        fileParseName,
		Description: "Parse a supported file inside the current working-directory boundary and return bounded text. The file is not modified.",
		InputSchema: append(json.RawMessage(nil), fileParseSchema...),
	}
}

func (c *Chat) parseFileTool(
	ctx context.Context,
	conversation store.Conversation,
	arguments json.RawMessage,
) (json.RawMessage, bool) {
	payload, err := parseConversationFile(ctx, conversation.CWD, arguments)
	if err != nil {
		encoded, _ := json.Marshal(map[string]string{"error": err.Error()})
		return encoded, false
	}
	encoded, err := json.Marshal(payload)
	if err != nil {
		return json.RawMessage(`{"error":"file parse result could not be encoded"}`), false
	}
	return encoded, true
}

func parseConversationFile(
	ctx context.Context,
	cwd string,
	arguments json.RawMessage,
) (fileParseResponse, error) {
	request, err := parseFileArguments(arguments)
	if err != nil {
		return fileParseResponse{}, err
	}
	if cwd == "" {
		return fileParseResponse{}, errors.New("conversation working directory is unavailable")
	}
	file, err := openConversationFile(cwd, request.path)
	if err != nil {
		return fileParseResponse{}, err
	}
	defer file.Close()
	return parseOpenFile(ctx, file, request.path, request.maxChars), nil
}

func parseFileArguments(arguments json.RawMessage) (fileParseRequest, error) {
	var fields map[string]json.RawMessage
	if err := decodeToolArguments(arguments, &fields); err != nil || len(fields) < 1 || len(fields) > 2 {
		return fileParseRequest{}, errors.New("arguments do not match the file.parse schema")
	}
	for name := range fields {
		if name != "path" && name != "max_chars" {
			return fileParseRequest{}, errors.New("arguments do not match the file.parse schema")
		}
	}
	var path string
	if json.Unmarshal(fields["path"], &path) != nil {
		return fileParseRequest{}, errors.New("arguments do not match the file.parse schema")
	}
	path = strings.TrimSpace(path)
	if path == "" {
		return fileParseRequest{}, errors.New("path is required")
	}
	if utf8.RuneCountInString(path) > fileParseMaximumPath {
		return fileParseRequest{}, errors.New("path must be 4096 characters or fewer")
	}
	maximum := fileParseMaximumCharacters
	if raw, exists := fields["max_chars"]; exists {
		if json.Unmarshal(raw, &maximum) != nil ||
			maximum < fileParseMinimumCharacters || maximum > fileParseMaximumCharacters {
			return fileParseRequest{}, errors.New("arguments do not match the file.parse schema")
		}
	}
	return fileParseRequest{path: path, maxChars: maximum}, nil
}

func openConversationFile(cwd, supplied string) (*os.File, error) {
	relative, err := normalizedFilePath(supplied)
	if err != nil {
		return nil, err
	}
	root, err := os.OpenRoot(cwd)
	if err != nil {
		return nil, errors.New("working directory is unavailable")
	}
	defer root.Close()
	current := ""
	for _, part := range strings.Split(filepath.ToSlash(relative), "/") {
		current = filepath.Join(current, part)
		metadata, err := root.Lstat(current)
		if err != nil {
			return nil, errors.New("file is unavailable")
		}
		if metadata.Mode()&os.ModeSymlink != 0 {
			return nil, errors.New("file path contains a symbolic link")
		}
	}
	file, err := root.Open(relative)
	if err != nil {
		return nil, errors.New("file could not be opened")
	}
	metadata, err := file.Stat()
	if err != nil {
		file.Close()
		return nil, errors.New("file is unavailable")
	}
	if !metadata.Mode().IsRegular() {
		file.Close()
		return nil, errors.New("file is not a regular file")
	}
	return file, nil
}

func normalizedFilePath(supplied string) (string, error) {
	if supplied == "" || filepath.IsAbs(supplied) || filepath.VolumeName(supplied) != "" {
		return "", errors.New("file path is outside the working-directory boundary")
	}
	for _, part := range strings.Split(filepath.ToSlash(supplied), "/") {
		if part == ".." {
			return "", errors.New("file path is outside the working-directory boundary")
		}
	}
	relative := filepath.Clean(supplied)
	if relative == "." {
		return "", errors.New("file path is required")
	}
	return relative, nil
}

func parseOpenFile(ctx context.Context, file *os.File, displayPath string, maxChars int) fileParseResponse {
	metadata, err := file.Stat()
	if err != nil || !metadata.Mode().IsRegular() {
		return failedFileParse(displayPath, 0, nil, "invalid_file")
	}
	format := fileFormat(displayPath)
	if isTextFile(format) {
		return parseTextFile(file, displayPath, metadata.Size(), format, maxChars)
	}
	if metadata.Size() > fileParseMaximumInput {
		return failedFileParse(displayPath, metadata.Size(), stringAddress(format), "source_too_large")
	}
	content, err := io.ReadAll(file)
	if err != nil {
		return failedFileParse(displayPath, metadata.Size(), stringAddress(format), "read_failed")
	}
	mediaType, detectedFormat := documentMedia(content, format)
	if mediaType == "" {
		return unsupportedFileParse(displayPath, metadata.Size(), format)
	}
	parseContext, cancel := context.WithTimeout(ctx, fileParseTimeout)
	defer cancel()
	conversion, timedOut := convertDocument(parseContext, content, mediaType)
	if timedOut {
		return failedFileParse(displayPath, metadata.Size(), nil, "worker_timeout")
	}
	if conversion.ErrorCode != "" {
		return failedFileParse(displayPath, metadata.Size(), nil, conversion.ErrorCode)
	}
	parser, markdown := "anydoc", "markdown"
	if !conversion.Converted {
		errorCode := "malformed"
		return fileParseResponse{
			Path: displayPath, SourceBytes: metadata.Size(), Status: "failed",
			Parser: &parser, Format: &detectedFormat, Error: &errorCode,
		}
	}
	observed := utf8.RuneCountInString(conversion.Content)
	bounded := truncateRunes(conversion.Content, maxChars)
	returned := utf8.RuneCountInString(bounded)
	return fileParseResponse{
		Path: displayPath, SourceBytes: metadata.Size(), Status: "converted",
		Parser: &parser, Format: &detectedFormat, ContentFormat: &markdown, Content: &bounded,
		ReturnedChars: returned, Truncated: observed > returned,
	}
}

func convertDocument(ctx context.Context, content []byte, mediaType string) (documentConversion, bool) {
	select {
	case documentParsePermit <- struct{}{}:
	case <-ctx.Done():
		return documentConversion{}, true
	}
	defer func() { <-documentParsePermit }()
	executable, err := os.Executable()
	if err != nil {
		return documentConversion{ErrorCode: "worker_unavailable"}, false
	}
	command := exec.CommandContext(ctx, executable)
	command.Env = []string{
		fileParseWorkerEnvironment + "=1",
		fileParseMediaEnvironment + "=" + mediaType,
	}
	command.Stdin = bytes.NewReader(content)
	command.Stderr = io.Discard
	output, err := command.Output()
	if ctx.Err() != nil {
		return documentConversion{}, true
	}
	if err != nil {
		return documentConversion{ErrorCode: "worker_failed"}, false
	}
	var converted documentConversion
	if len(output) > fileParseMaximumCharacters*6+1_024 || json.Unmarshal(output, &converted) != nil {
		return documentConversion{ErrorCode: "worker_failed"}, false
	}
	return converted, false
}

// RunFileParseWorkerIfRequested runs the private bounded document converter.
// The process entrypoint must exit with the returned status when handled is true.
func RunFileParseWorkerIfRequested() (handled bool, status int) {
	if os.Getenv(fileParseWorkerEnvironment) != "1" {
		return false, 0
	}
	if !limitFileParseWorkerMemory(fileParseWorkerMemory) {
		return true, 2
	}
	mediaType := os.Getenv(fileParseMediaEnvironment)
	if !documents.IsDocument(mediaType) {
		return true, 2
	}
	content, err := io.ReadAll(io.LimitReader(os.Stdin, fileParseMaximumInput+1))
	if err != nil || len(content) > fileParseMaximumInput {
		return true, 2
	}
	markdown, converted := documents.DocumentMarkdown(content, mediaType)
	response, err := json.Marshal(documentConversion{Content: markdown, Converted: converted})
	if err != nil || len(response) > fileParseMaximumCharacters*6+1_024 {
		return true, 2
	}
	if _, err = os.Stdout.Write(response); err != nil {
		return true, 2
	}
	return true, 0
}

func parseTextFile(file *os.File, path string, sourceBytes int64, format string, maxChars int) fileParseResponse {
	byteLimit := int64((maxChars+1)*utf8.UTFMax + utf8.UTFMax - 1)
	content, err := io.ReadAll(io.LimitReader(file, byteLimit))
	if err != nil {
		return failedFileParse(path, sourceBytes, stringAddress(format), "read_failed")
	}
	sourceHasMore := sourceBytes > int64(len(content))
	text, valid := boundedUTF8Text(content, sourceHasMore)
	if !valid {
		return failedFileParse(path, sourceBytes, stringAddress(format), "invalid_utf8")
	}
	observed := utf8.RuneCountInString(text)
	bounded := truncateRunes(text, maxChars)
	returned := utf8.RuneCountInString(bounded)
	parser, contentFormat := "utf8", "text"
	if format == "csv" {
		contentFormat = "csv"
	} else if format == "md" || format == "markdown" {
		contentFormat = "markdown"
	}
	return fileParseResponse{
		Path: path, SourceBytes: sourceBytes, Status: "converted", Parser: &parser,
		Format: stringAddress(format), ContentFormat: &contentFormat, Content: &bounded,
		ReturnedChars: returned, Truncated: sourceHasMore || observed > returned,
	}
}

func boundedUTF8Text(content []byte, sourceHasMore bool) (string, bool) {
	if utf8.Valid(content) {
		return string(content), true
	}
	for offset := 0; offset < len(content); {
		character, size := utf8.DecodeRune(content[offset:])
		if character == utf8.RuneError && size == 1 {
			if sourceHasMore && !utf8.FullRune(content[offset:]) {
				return string(content[:offset]), true
			}
			return "", false
		}
		offset += size
	}
	return "", false
}

func documentMedia(content []byte, format string) (string, string) {
	if index := bytes.Index(content[:min(len(content), 1_024)], []byte("%PDF-")); index >= 0 {
		return documents.MediaPDF, "pdf"
	}
	if bytes.HasPrefix(bytes.TrimSpace(content), []byte(`{\rtf`)) {
		return documents.MediaRTF, "rtf"
	}
	if compound, err := cfb.NewReader(bytes.NewReader(content)); err == nil {
		if _, err = compound.OpenStream("PowerPoint Document"); err == nil {
			return documents.MediaPPT, "ppt"
		}
	}
	if mediaType, name := documentMediaFromExtension(format); mediaType != "" {
		return mediaType, name
	}
	archive, err := zip.NewReader(bytes.NewReader(content), int64(len(content)))
	if err != nil || len(archive.File) > 10_000 {
		return "", format
	}
	for _, file := range archive.File {
		switch filepath.ToSlash(file.Name) {
		case "mimetype":
			reader, openError := file.Open()
			if openError != nil {
				continue
			}
			value, readError := io.ReadAll(io.LimitReader(reader, 256))
			reader.Close()
			if readError == nil {
				switch strings.TrimSpace(string(value)) {
				case documents.MediaODT:
					return documents.MediaODT, "odt"
				case documents.MediaODS:
					return documents.MediaODS, "ods"
				case documents.MediaODP:
					return documents.MediaODP, "odp"
				case documents.MediaEPUB:
					return documents.MediaEPUB, "epub"
				}
			}
		case "word/document.xml":
			return documents.MediaDOCX, "docx"
		case "ppt/presentation.xml":
			return documents.MediaPPTX, "pptx"
		case "xl/workbook.xml":
			return documents.MediaXLSX, "excel"
		case "META-INF/container.xml":
			return documents.MediaEPUB, "epub"
		}
	}
	return "", format
}

func documentMediaFromExtension(format string) (string, string) {
	switch format {
	case "xls":
		return documents.MediaXLS, "excel"
	case "xlsx":
		return documents.MediaXLSX, "excel"
	case "ods":
		return documents.MediaODS, "ods"
	case "docx":
		return documents.MediaDOCX, "docx"
	case "odt":
		return documents.MediaODT, "odt"
	case "pptx":
		return documents.MediaPPTX, "pptx"
	case "ppt", "pps", "pot":
		return documents.MediaPPT, "ppt"
	case "odp":
		return documents.MediaODP, "odp"
	case "rtf":
		return documents.MediaRTF, "rtf"
	case "pdf":
		return documents.MediaPDF, "pdf"
	case "epub":
		return documents.MediaEPUB, "epub"
	default:
		return "", format
	}
}

func fileFormat(path string) string {
	extension := strings.TrimPrefix(strings.ToLower(filepath.Ext(path)), ".")
	return extension
}

func isTextFile(format string) bool {
	switch format {
	case "csv", "txt", "text", "md", "markdown", "eml", "json", "xml":
		return true
	default:
		return false
	}
}

func unsupportedFileParse(path string, sourceBytes int64, format string) fileParseResponse {
	parser, errorCode := "anydoc", "unsupported_format"
	return fileParseResponse{
		Path: path, SourceBytes: sourceBytes, Status: "unsupported", Parser: &parser,
		Format: stringAddress(format), Error: &errorCode,
	}
}

func failedFileParse(path string, sourceBytes int64, format *string, code string) fileParseResponse {
	return fileParseResponse{
		Path: path, SourceBytes: sourceBytes, Status: "failed", Format: format, Error: &code,
	}
}

func truncateRunes(content string, maximum int) string {
	if utf8.RuneCountInString(content) <= maximum {
		return content
	}
	return string([]rune(content)[:maximum])
}

func stringAddress(value string) *string {
	if value == "" {
		return nil
	}
	copy := value
	return &copy
}
