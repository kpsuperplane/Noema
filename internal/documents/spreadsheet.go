// Package documents converts bounded document content for local previews.
package documents

import (
	"archive/zip"
	"bytes"
	"encoding/xml"
	"errors"
	"fmt"
	"io"
	"path"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/Clownsw/xls"
)

const (
	MediaXLSX = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
	MediaXLS  = "application/vnd.ms-excel"
	MediaODS  = "application/vnd.oasis.opendocument.spreadsheet"

	maxInputBytes        = 8 * 1024 * 1024
	maxArchiveEntries    = 512
	maxArchivePartBytes  = 8 * 1024 * 1024
	maxArchiveTotalBytes = 32 * 1024 * 1024
	maxPreviewCharacters = 20_000
	maxSheets            = 64
	maxRows              = 2_000
	maxColumns           = 256
	maxCells             = 50_000
)

var errInvalidSpreadsheet = errors.New("invalid spreadsheet")

type sheet struct {
	name       string
	rows       [][]string
	headerRows int
}

// IsSpreadsheet reports whether a media type has a supported spreadsheet parser.
func IsSpreadsheet(mediaType string) bool {
	switch normalizedMediaType(mediaType) {
	case MediaXLSX, MediaXLS, MediaODS:
		return true
	default:
		return false
	}
}

// SpreadsheetMarkdown converts one supported spreadsheet into bounded Markdown.
// Malformed, encrypted, and resource-heavy inputs produce no parser diagnostics.
func SpreadsheetMarkdown(data []byte, mediaType string) (content string, converted bool) {
	if len(data) == 0 || len(data) > maxInputBytes {
		return "", false
	}
	defer func() {
		if recover() != nil {
			content, converted = "", false
		}
	}()

	var parsed []sheet
	var err error
	switch normalizedMediaType(mediaType) {
	case MediaXLSX:
		parsed, err = parseXLSX(data)
	case MediaXLS:
		parsed, err = parseXLS(data)
	case MediaODS:
		parsed, err = parseODS(data)
	default:
		return "", false
	}
	if err != nil {
		return "", false
	}
	return renderMarkdown(parsed), true
}

func normalizedMediaType(value string) string {
	value, _, _ = strings.Cut(value, ";")
	return strings.ToLower(strings.TrimSpace(value))
}

func parseXLS(data []byte) ([]sheet, error) {
	book, err := xls.OpenReader(bytes.NewReader(data))
	if err != nil || book.NumSheets() == 0 || book.NumSheets() > maxSheets {
		return nil, errInvalidSpreadsheet
	}
	result := make([]sheet, 0, book.NumSheets())
	cells := 0
	for index := 0; index < book.NumSheets() && index < maxSheets; index++ {
		worksheet := book.GetSheet(index)
		if worksheet == nil {
			continue
		}
		current := sheet{name: worksheet.Name}
		rowCount := min(int(worksheet.MaxRow)+1, maxRows)
		for rowIndex := 0; rowIndex < rowCount && cells < maxCells; rowIndex++ {
			row := worksheet.Row(rowIndex)
			if row == nil {
				current.rows = append(current.rows, nil)
				continue
			}
			width := min(row.LastCol(), maxColumns)
			values := make([]string, width)
			for column := max(0, row.FirstCol()); column < width && cells < maxCells; column++ {
				values[column] = row.ColExact(column)
				cells++
			}
			current.rows = append(current.rows, values)
		}
		result = append(result, current)
	}
	return result, nil
}

func parseXLSX(data []byte) ([]sheet, error) {
	archive, entries, err := openBoundedArchive(data)
	if err != nil {
		return nil, err
	}
	workbook, err := readArchivePart(archive, entries, "xl/workbook.xml")
	if err != nil {
		return nil, err
	}
	relations, err := readArchivePart(archive, entries, "xl/_rels/workbook.xml.rels")
	if err != nil {
		return nil, err
	}
	names, err := workbookSheets(workbook)
	if err != nil || len(names) == 0 || len(names) > maxSheets {
		return nil, errInvalidSpreadsheet
	}
	targets, err := workbookRelations(relations)
	if err != nil {
		return nil, err
	}
	var shared []string
	if _, exists := entries["xl/sharedStrings.xml"]; exists {
		part, readErr := readArchivePart(archive, entries, "xl/sharedStrings.xml")
		if readErr != nil {
			return nil, readErr
		}
		shared, err = sharedStrings(part)
		if err != nil {
			return nil, err
		}
	}
	result := make([]sheet, 0, len(names))
	cells := 0
	for _, named := range names {
		target, exists := targets[named.id]
		if !exists {
			return nil, errInvalidSpreadsheet
		}
		partName, ok := safeWorkbookTarget(target)
		if !ok {
			return nil, errInvalidSpreadsheet
		}
		part, err := readArchivePart(archive, entries, partName)
		if err != nil {
			return nil, err
		}
		rows, err := xlsxRows(part, shared, &cells)
		if err != nil {
			return nil, err
		}
		result = append(result, sheet{name: named.name, rows: rows})
		if cells >= maxCells {
			break
		}
	}
	return result, nil
}

func parseODS(data []byte) ([]sheet, error) {
	archive, entries, err := openBoundedArchive(data)
	if err != nil {
		return nil, err
	}
	part, err := readArchivePart(archive, entries, "content.xml")
	if err != nil {
		return nil, err
	}
	decoder := xml.NewDecoder(bytes.NewReader(part))
	result := make([]sheet, 0)
	cells := 0
	for {
		token, tokenErr := decoder.Token()
		if tokenErr == io.EOF {
			break
		}
		if tokenErr != nil {
			return nil, errInvalidSpreadsheet
		}
		start, ok := token.(xml.StartElement)
		if !ok || start.Name.Local != "table" {
			continue
		}
		if len(result) >= maxSheets {
			return nil, errInvalidSpreadsheet
		}
		parsed, err := odsSheet(decoder, start, &cells)
		if err != nil {
			return nil, err
		}
		result = append(result, parsed)
	}
	if len(result) == 0 {
		return nil, errInvalidSpreadsheet
	}
	return result, nil
}

type archiveEntries map[string]*zip.File

func openBoundedArchive(data []byte) (*zip.Reader, archiveEntries, error) {
	archive, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil || len(archive.File) > maxArchiveEntries {
		return nil, nil, errInvalidSpreadsheet
	}
	entries := make(archiveEntries, len(archive.File))
	var total uint64
	for _, file := range archive.File {
		if file.UncompressedSize64 > maxArchivePartBytes {
			return nil, nil, errInvalidSpreadsheet
		}
		total += file.UncompressedSize64
		if total > maxArchiveTotalBytes {
			return nil, nil, errInvalidSpreadsheet
		}
		name := path.Clean(strings.TrimPrefix(file.Name, "/"))
		if name == "." || name == ".." || strings.HasPrefix(name, "../") {
			return nil, nil, errInvalidSpreadsheet
		}
		entries[name] = file
	}
	return archive, entries, nil
}

func readArchivePart(_ *zip.Reader, entries archiveEntries, name string) ([]byte, error) {
	file, ok := entries[name]
	if !ok {
		return nil, errInvalidSpreadsheet
	}
	reader, err := file.Open()
	if err != nil {
		return nil, errInvalidSpreadsheet
	}
	defer reader.Close()
	content, err := io.ReadAll(io.LimitReader(reader, maxArchivePartBytes+1))
	if err != nil || len(content) > maxArchivePartBytes {
		return nil, errInvalidSpreadsheet
	}
	return content, nil
}

type namedSheet struct {
	name string
	id   string
}

func workbookSheets(content []byte) ([]namedSheet, error) {
	decoder := xml.NewDecoder(bytes.NewReader(content))
	var result []namedSheet
	for {
		token, err := decoder.Token()
		if err == io.EOF {
			return result, nil
		}
		if err != nil {
			return nil, errInvalidSpreadsheet
		}
		start, ok := token.(xml.StartElement)
		if !ok || start.Name.Local != "sheet" {
			continue
		}
		result = append(result, namedSheet{name: attribute(start, "name"), id: attribute(start, "id")})
	}
}

func workbookRelations(content []byte) (map[string]string, error) {
	decoder := xml.NewDecoder(bytes.NewReader(content))
	result := make(map[string]string)
	for {
		token, err := decoder.Token()
		if err == io.EOF {
			return result, nil
		}
		if err != nil {
			return nil, errInvalidSpreadsheet
		}
		start, ok := token.(xml.StartElement)
		if !ok || start.Name.Local != "Relationship" {
			continue
		}
		result[attribute(start, "Id")] = attribute(start, "Target")
	}
}

func safeWorkbookTarget(target string) (string, bool) {
	if strings.HasPrefix(target, "/") {
		target = strings.TrimPrefix(target, "/")
	} else {
		target = path.Join("xl", target)
	}
	target = path.Clean(target)
	return target, target != "xl" && strings.HasPrefix(target, "xl/")
}

func sharedStrings(content []byte) ([]string, error) {
	decoder := xml.NewDecoder(bytes.NewReader(content))
	result := make([]string, 0)
	for {
		token, err := decoder.Token()
		if err == io.EOF {
			return result, nil
		}
		if err != nil {
			return nil, errInvalidSpreadsheet
		}
		start, ok := token.(xml.StartElement)
		if !ok || start.Name.Local != "si" {
			continue
		}
		text, err := xlsxString(decoder, start)
		if err != nil || len(result) >= maxCells {
			return nil, errInvalidSpreadsheet
		}
		result = append(result, text)
	}
}

func xlsxString(decoder *xml.Decoder, start xml.StartElement) (string, error) {
	var text strings.Builder
	for {
		token, err := decoder.Token()
		if err != nil {
			return "", errInvalidSpreadsheet
		}
		switch typed := token.(type) {
		case xml.StartElement:
			if typed.Name.Local == "t" {
				part, err := elementText(decoder, typed)
				if err != nil {
					return "", err
				}
				text.WriteString(part)
			}
		case xml.EndElement:
			if typed.Name == start.Name {
				return text.String(), nil
			}
		}
	}
}

func xlsxRows(content []byte, shared []string, cellCount *int) ([][]string, error) {
	decoder := xml.NewDecoder(bytes.NewReader(content))
	rows := make([][]string, 0)
	var row []string
	for {
		token, err := decoder.Token()
		if err == io.EOF {
			return rows, nil
		}
		if err != nil {
			return nil, errInvalidSpreadsheet
		}
		switch typed := token.(type) {
		case xml.StartElement:
			switch typed.Name.Local {
			case "row":
				if len(rows) >= maxRows || *cellCount >= maxCells {
					return rows, nil
				}
				row = nil
			case "c":
				if row == nil {
					row = make([]string, 0)
				}
				column, value, err := xlsxCell(decoder, typed, shared)
				if err != nil {
					return nil, err
				}
				if column < 0 {
					column = len(row)
				}
				if column < maxColumns {
					for len(row) <= column {
						row = append(row, "")
					}
					row[column] = value
					(*cellCount)++
				}
			}
		case xml.EndElement:
			if typed.Name.Local == "row" {
				rows = append(rows, row)
			}
		}
	}
}

func xlsxCell(decoder *xml.Decoder, start xml.StartElement, shared []string) (int, string, error) {
	column := columnFromReference(attribute(start, "r"))
	cellType := attribute(start, "t")
	var value, inline strings.Builder
	for {
		token, err := decoder.Token()
		if err != nil {
			return 0, "", errInvalidSpreadsheet
		}
		switch typed := token.(type) {
		case xml.StartElement:
			if typed.Name.Local == "v" {
				text, err := elementText(decoder, typed)
				if err != nil {
					return 0, "", err
				}
				value.WriteString(text)
			} else if typed.Name.Local == "t" {
				text, err := elementText(decoder, typed)
				if err != nil {
					return 0, "", err
				}
				inline.WriteString(text)
			}
		case xml.EndElement:
			if typed.Name == start.Name {
				raw := value.String()
				switch cellType {
				case "s":
					index, err := strconv.Atoi(strings.TrimSpace(raw))
					if err != nil || index < 0 || index >= len(shared) {
						return 0, "", errInvalidSpreadsheet
					}
					return column, shared[index], nil
				case "b":
					if strings.TrimSpace(raw) == "1" {
						return column, "TRUE", nil
					}
					return column, "FALSE", nil
				case "inlineStr":
					return column, inline.String(), nil
				default:
					return column, raw, nil
				}
			}
		}
	}
}

func odsSheet(decoder *xml.Decoder, start xml.StartElement, cellCount *int) (sheet, error) {
	result := sheet{name: attribute(start, "name")}
	inHeader := false
	for {
		token, err := decoder.Token()
		if err != nil {
			return sheet{}, errInvalidSpreadsheet
		}
		switch typed := token.(type) {
		case xml.StartElement:
			if typed.Name.Local == "table-header-rows" {
				inHeader = true
				continue
			}
			if typed.Name.Local != "table-row" {
				continue
			}
			row, repeat, err := odsRow(decoder, typed, cellCount)
			if err != nil {
				return sheet{}, err
			}
			emitted := min(repeat, maxRows-len(result.rows))
			for range emitted {
				result.rows = append(result.rows, append([]string(nil), row...))
			}
			if inHeader {
				result.headerRows += emitted
			}
			if len(result.rows) >= maxRows || *cellCount >= maxCells {
				return result, nil
			}
		case xml.EndElement:
			if typed.Name.Local == "table-header-rows" {
				inHeader = false
				continue
			}
			if typed.Name == start.Name {
				return result, nil
			}
		}
	}
}

func odsRow(decoder *xml.Decoder, start xml.StartElement, cellCount *int) ([]string, int, error) {
	repeat := positiveAttribute(start, "number-rows-repeated")
	row := make([]string, 0)
	for {
		token, err := decoder.Token()
		if err != nil {
			return nil, 0, errInvalidSpreadsheet
		}
		switch typed := token.(type) {
		case xml.StartElement:
			if typed.Name.Local != "table-cell" && typed.Name.Local != "covered-table-cell" {
				continue
			}
			value, cellRepeat, err := odsCell(decoder, typed)
			if err != nil {
				return nil, 0, err
			}
			for range min(cellRepeat, maxColumns-len(row)) {
				row = append(row, value)
				(*cellCount)++
			}
		case xml.EndElement:
			if typed.Name == start.Name {
				return row, repeat, nil
			}
		}
	}
}

func odsCell(decoder *xml.Decoder, start xml.StartElement) (string, int, error) {
	repeat := positiveAttribute(start, "number-columns-repeated")
	valueType := attribute(start, "value-type")
	fallback := odsValue(start, valueType)
	text, err := odsCellText(decoder, start)
	if err != nil {
		return "", 0, err
	}
	if strings.TrimSpace(text) == "" {
		text = fallback
	}
	return text, repeat, nil
}

func odsCellText(decoder *xml.Decoder, start xml.StartElement) (string, error) {
	paragraphs := make([]string, 0, 1)
	for {
		token, err := decoder.Token()
		if err != nil {
			return "", errInvalidSpreadsheet
		}
		switch typed := token.(type) {
		case xml.StartElement:
			if typed.Name.Local == "p" || typed.Name.Local == "h" {
				paragraph, err := elementText(decoder, typed)
				if err != nil {
					return "", err
				}
				if strings.TrimSpace(paragraph) != "" {
					paragraphs = append(paragraphs, paragraph)
				}
			}
		case xml.EndElement:
			if typed.Name == start.Name {
				return strings.Join(paragraphs, "\n"), nil
			}
		}
	}
}

func odsValue(start xml.StartElement, valueType string) string {
	switch valueType {
	case "boolean":
		if attribute(start, "boolean-value") == "true" {
			return "TRUE"
		}
		return "FALSE"
	case "date":
		return attribute(start, "date-value")
	case "time":
		return attribute(start, "time-value")
	case "string":
		return attribute(start, "string-value")
	case "percentage":
		value, err := strconv.ParseFloat(attribute(start, "value"), 64)
		if err == nil {
			return fmt.Sprintf("%g%%", value*100)
		}
	case "currency":
		value := attribute(start, "value")
		if currency := attribute(start, "currency"); currency != "" {
			return value + " " + currency
		}
		return value
	case "float":
		value, err := strconv.ParseFloat(attribute(start, "value"), 64)
		if err == nil {
			return strconv.FormatFloat(value, 'g', -1, 64)
		}
	}
	return ""
}

func elementText(decoder *xml.Decoder, start xml.StartElement) (string, error) {
	var text strings.Builder
	depth := 1
	for depth > 0 {
		token, err := decoder.Token()
		if err != nil {
			return "", errInvalidSpreadsheet
		}
		switch typed := token.(type) {
		case xml.StartElement:
			depth++
		case xml.EndElement:
			depth--
		case xml.CharData:
			if text.Len()+len(typed) > maxArchivePartBytes {
				return "", errInvalidSpreadsheet
			}
			text.Write(typed)
		}
	}
	return text.String(), nil
}

func attribute(start xml.StartElement, name string) string {
	for _, attribute := range start.Attr {
		if attribute.Name.Local == name {
			return attribute.Value
		}
	}
	return ""
}

func positiveAttribute(start xml.StartElement, name string) int {
	value, err := strconv.Atoi(attribute(start, name))
	if err != nil || value < 1 {
		return 1
	}
	return value
}

func columnFromReference(reference string) int {
	column := 0
	found := false
	for _, character := range reference {
		if character < 'A' || character > 'Z' {
			break
		}
		found = true
		column = column*26 + int(character-'A'+1)
		if column > maxColumns {
			return maxColumns
		}
	}
	if !found {
		return -1
	}
	return column - 1
}

func renderMarkdown(sheets []sheet) string {
	multiple := len(sheets) > 1
	var output strings.Builder
	for _, sheet := range sheets {
		rows := compactRows(sheet.rows)
		if len(rows) == 0 {
			continue
		}
		if multiple {
			appendBounded(&output, "## "+escapeInline(sheet.name)+"\n\n")
		}
		width := 0
		for _, row := range rows {
			width = max(width, len(row))
		}
		header := sheet.headerRows > 0 || inferredHeader(rows)
		if !header {
			appendBounded(&output, "| "+strings.TrimSuffix(strings.Repeat(" | ", width), " ")+"\n")
			appendBounded(&output, "| "+strings.TrimSuffix(strings.Repeat("--- | ", width), " ")+"\n")
		}
		for index, row := range rows {
			values := make([]string, width)
			for column := range width {
				if column < len(row) {
					values[column] = escapeCell(row[column])
				}
			}
			appendBounded(&output, "| "+strings.Join(values, " | ")+" |\n")
			if header && index == 0 {
				appendBounded(&output, "| "+strings.TrimSuffix(strings.Repeat("--- | ", width), " ")+"\n")
			}
			if utf8.RuneCountInString(output.String()) >= maxPreviewCharacters {
				return output.String()
			}
		}
	}
	return strings.TrimSuffix(output.String(), "\n")
}

func inferredHeader(rows [][]string) bool {
	if len(rows) < 2 {
		return false
	}
	widths := make(map[int]int)
	for _, row := range rows[1:min(len(rows), 51)] {
		widths[len(row)]++
	}
	modalWidth, modalCount := 0, 0
	for width, count := range widths {
		if count > modalCount || count == modalCount && width > modalWidth {
			modalWidth, modalCount = width, count
		}
	}
	if len(rows[0]) != modalWidth {
		return false
	}
	seen := make(map[string]struct{}, modalWidth)
	for column, value := range rows[0] {
		label := strings.ToLower(strings.TrimSpace(value))
		if label == "" && column != 0 || strings.Contains(value, "\n") || utf8.RuneCountInString(value) > 64 {
			return false
		}
		if label != "" {
			if _, duplicate := seen[label]; duplicate {
				return false
			}
			seen[label] = struct{}{}
		}
	}
	headerVotes, dataVotes := 0, 0
	for column, label := range rows[0] {
		values := make([]string, 0, len(rows)-1)
		for _, row := range rows[1:min(len(rows), 51)] {
			if column < len(row) && strings.TrimSpace(row[column]) != "" {
				values = append(values, strings.TrimSpace(row[column]))
			}
		}
		if len(values) == 0 {
			continue
		}
		kind, dominant := dominantCellKind(values)
		if dominant && kind != cellText {
			if classifyCell(label) == cellText {
				headerVotes++
			} else {
				dataVotes++
			}
			continue
		}
		for _, value := range values {
			if strings.EqualFold(strings.TrimSpace(label), value) {
				dataVotes++
				break
			}
		}
	}
	if headerVotes == 0 && dataVotes == 0 {
		return true
	}
	return headerVotes > dataVotes
}

type cellKind uint8

const (
	cellText cellKind = iota
	cellNumber
	cellBoolean
)

func dominantCellKind(values []string) (cellKind, bool) {
	for _, kind := range []cellKind{cellNumber, cellBoolean, cellText} {
		matches := 0
		for _, value := range values {
			if classifyCell(value) == kind {
				matches++
			}
		}
		if matches*10 >= len(values)*9 {
			return kind, true
		}
	}
	return cellText, false
}

func classifyCell(value string) cellKind {
	value = strings.TrimSpace(value)
	if value != "" {
		cleaned := strings.NewReplacer(",", "", " ", "", "_", "", "\u00a0", "").Replace(strings.TrimSuffix(value, "%"))
		if strings.IndexFunc(cleaned, func(character rune) bool { return character >= '0' && character <= '9' }) >= 0 {
			if _, err := strconv.ParseFloat(cleaned, 64); err == nil {
				return cellNumber
			}
		}
	}
	if strings.EqualFold(value, "true") || strings.EqualFold(value, "false") ||
		strings.EqualFold(value, "yes") || strings.EqualFold(value, "no") {
		return cellBoolean
	}
	return cellText
}

func compactRows(rows [][]string) [][]string {
	for len(rows) > 0 && rowEmpty(rows[len(rows)-1]) {
		rows = rows[:len(rows)-1]
	}
	width := 0
	for _, row := range rows {
		for index := len(row) - 1; index >= 0; index-- {
			if strings.TrimSpace(row[index]) != "" {
				width = max(width, index+1)
				break
			}
		}
	}
	if width == 0 {
		return nil
	}
	for index := range rows {
		rows[index] = rows[index][:min(len(rows[index]), width)]
		rows[index] = append(rows[index], make([]string, width-len(rows[index]))...)
	}
	return rows
}

func rowEmpty(row []string) bool {
	for _, value := range row {
		if strings.TrimSpace(value) != "" {
			return false
		}
	}
	return true
}

func escapeCell(value string) string {
	value = strings.ReplaceAll(value, "\r\n", "\n")
	value = strings.ReplaceAll(value, "\r", "\n")
	lines := strings.Split(value, "\n")
	for index := range lines {
		lines[index] = escapeMarkdown(strings.TrimSpace(lines[index]), true)
	}
	return strings.Join(lines, "<br>")
}

func escapeInline(value string) string {
	return escapeMarkdown(strings.TrimSpace(value), false)
}

func escapeMarkdown(value string, tableCell bool) string {
	var escaped strings.Builder
	for _, character := range value {
		if strings.ContainsRune("\\`*_~[]<!", character) || tableCell && character == '|' {
			escaped.WriteByte('\\')
		}
		escaped.WriteRune(character)
	}
	return escaped.String()
}

func appendBounded(output *strings.Builder, value string) {
	remaining := maxPreviewCharacters - utf8.RuneCountInString(output.String())
	if remaining <= 0 {
		return
	}
	for _, character := range value {
		if remaining == 0 {
			return
		}
		output.WriteRune(character)
		remaining--
	}
}
