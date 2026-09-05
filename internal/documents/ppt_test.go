package documents

import (
	"bytes"
	"encoding/binary"
	"os"
	"strings"
	"testing"

	"github.com/abemedia/go-cfb"
)

func pptRecordBytes(version, kind uint16, body []byte) []byte {
	result := make([]byte, 8+len(body))
	binary.LittleEndian.PutUint16(result, version)
	binary.LittleEndian.PutUint16(result[2:], kind)
	binary.LittleEndian.PutUint32(result[4:], uint32(len(body)))
	copy(result[8:], body)
	return result
}

func pptAtom(kind uint16, body []byte) []byte { return pptRecordBytes(0, kind, body) }

func pptContainer(instance, kind uint16, children ...[]byte) []byte {
	return pptRecordBytes(instance<<4|0x000f, kind, bytes.Join(children, nil))
}

func pptText(textType byte, value string, style []byte) []byte {
	header := pptAtom(0x0f9f, []byte{textType})
	units := utf16Bytes(value)
	result := append(header, pptAtom(0x0fa0, units)...)
	if style != nil {
		result = append(result, pptAtom(0x0fa1, style)...)
	}
	return result
}

func utf16Bytes(value string) []byte {
	var result []byte
	for _, character := range value {
		if character <= 0xffff {
			result = binary.LittleEndian.AppendUint16(result, uint16(character))
			continue
		}
		character -= 0x10000
		result = binary.LittleEndian.AppendUint16(result, uint16(0xd800+(character>>10)))
		result = binary.LittleEndian.AppendUint16(result, uint16(0xdc00+(character&0x3ff)))
	}
	return result
}

func pptPersistAtom(persistID, pageID uint32) []byte {
	body := make([]byte, 16)
	binary.LittleEndian.PutUint32(body, persistID)
	binary.LittleEndian.PutUint32(body[12:], pageID)
	return pptAtom(0x03f3, body)
}

func pptCompound(t *testing.T, streams map[string][]byte) []byte {
	t.Helper()
	file, err := os.CreateTemp(t.TempDir(), "*.ppt")
	if err != nil {
		t.Fatal(err)
	}
	writer := cfb.NewWriterV3(file)
	for name, content := range streams {
		stream, createError := writer.CreateStream(name)
		if createError != nil {
			file.Close()
			t.Fatal(createError)
		}
		if _, err = stream.Write(content); err != nil {
			file.Close()
			t.Fatal(err)
		}
		if err = stream.Close(); err != nil {
			file.Close()
			t.Fatal(err)
		}
	}
	if err = writer.Close(); err != nil {
		file.Close()
		t.Fatal(err)
	}
	if err = file.Close(); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(file.Name())
	if err != nil {
		t.Fatal(err)
	}
	return content
}

func pptStyleFixture() []byte {
	var result []byte
	for _, paragraph := range []struct {
		count, depth uint16
	}{{4, 0}, {6, 1}} {
		result = binary.LittleEndian.AppendUint32(result, uint32(paragraph.count))
		result = binary.LittleEndian.AppendUint16(result, paragraph.depth)
		result = binary.LittleEndian.AppendUint32(result, 0)
	}
	for _, character := range []struct {
		count uint32
		style uint16
	}{{4, 1}, {6, 2}} {
		result = binary.LittleEndian.AppendUint32(result, character.count)
		result = binary.LittleEndian.AppendUint32(result, uint32(character.style))
		result = binary.LittleEndian.AppendUint16(result, character.style)
	}
	return result
}

func pptPersistedFixture(t *testing.T) []byte {
	t.Helper()
	masterStyle := binary.LittleEndian.AppendUint16(nil, 2)
	for range 2 {
		masterStyle = binary.LittleEndian.AppendUint32(masterStyle, 1)
		masterStyle = binary.LittleEndian.AppendUint16(masterStyle, 1)
		masterStyle = binary.LittleEndian.AppendUint32(masterStyle, 0)
	}
	master := pptContainer(0, 0x03f8, pptRecordBytes(1<<4, 0x0fa3, masterStyle))

	slideAtom := make([]byte, 16)
	binary.LittleEndian.PutUint32(slideAtom[12:], 77)
	slideOne := pptContainer(0, 0x03ee, pptAtom(0x03ef, slideAtom), pptText(2, "Textbox", nil))
	slideTwo := pptContainer(0, 0x03ee, pptText(0, "Second", nil))
	notesAtom := make([]byte, 4)
	binary.LittleEndian.PutUint32(notesAtom, 101)
	notes := pptContainer(0, 0x03f0, pptAtom(0x03f1, notesAtom), pptText(2, "Speaker one\rMore", nil))

	masterList := pptContainer(1, 0x0ff0, pptPersistAtom(5, 77))
	slideList := pptContainer(0, 0x0ff0,
		pptPersistAtom(2, 101), pptText(0, "First", nil), pptText(1, "Top\rChild", pptStyleFixture()),
		pptPersistAtom(3, 102), pptText(0, "Outline second", nil),
	)
	notesList := pptContainer(2, 0x0ff0, pptPersistAtom(4, 900))
	document := pptContainer(0, 0x03e8, masterList, slideList, notesList)

	data := pptAtom(0, nil)
	offsets := []int{len(data)}
	data = append(data, document...)
	for _, record := range [][]byte{slideOne, slideTwo, notes, master} {
		offsets = append(offsets, len(data))
		data = append(data, record...)
	}
	directoryOffset := len(data)
	directory := binary.LittleEndian.AppendUint32(nil, 1|5<<20)
	for _, offset := range offsets {
		directory = binary.LittleEndian.AppendUint32(directory, uint32(offset))
	}
	data = append(data, pptAtom(0x1772, directory)...)
	editOffset := len(data)
	edit := make([]byte, 20)
	binary.LittleEndian.PutUint32(edit[12:], uint32(directoryOffset))
	binary.LittleEndian.PutUint32(edit[16:], 1)
	data = append(data, pptAtom(0x0ff5, edit)...)
	currentUser := make([]byte, 20)
	binary.LittleEndian.PutUint32(currentUser[16:], uint32(editOffset))
	return pptCompound(t, map[string][]byte{"PowerPoint Document": data, "Current User": currentUser})
}

func TestDocumentMarkdownPPTKeepsPersistedStructure(t *testing.T) {
	got, converted := DocumentMarkdown(pptPersistedFixture(t), MediaPPT)
	want := "## First\n\n- **Top**\n\n  - *Child*\n\nTextbox\n\n> Speaker one\n>\n> More\n\n## Outline second\n\n## Second"
	if !converted || got != want {
		t.Fatalf("PPT converted = %v\n--- got ---\n%s\n--- want ---\n%s", converted, got, want)
	}
}

func TestDocumentMarkdownPPTRecoversRawTextAndNotes(t *testing.T) {
	note := pptContainer(0, 0x03f0, pptText(1, "Remember", nil))
	data := append(pptAtom(0x0fa8, []byte("Caf\xe9\vNext")), note...)
	got, converted := DocumentMarkdown(pptCompound(t, map[string][]byte{"PowerPoint Document": data}), MediaPPT)
	if !converted || got != "Café\\\nNext\n\n> Remember" {
		t.Fatalf("recovered PPT = %q, converted = %v", got, converted)
	}
}

func TestDocumentMarkdownPPTRejectsMalformedEncryptedAndHostileRecords(t *testing.T) {
	encryptedUser := make([]byte, 16)
	binary.LittleEndian.PutUint32(encryptedUser[12:], 0xf3d1c4df)
	for name, input := range map[string][]byte{
		"malformed": []byte("not a compound document"),
		"encrypted": pptCompound(t, map[string][]byte{"PowerPoint Document": pptAtom(0x0fa8, []byte("hidden")), "Current User": encryptedUser}),
	} {
		if got, converted := DocumentMarkdown(input, MediaPPT); converted || got != "" {
			t.Fatalf("%s PPT converted to %q", name, got)
		}
	}
	deep := pptAtom(0x0fa8, []byte("deep"))
	for range maxPPTRecordDepth {
		deep = pptContainer(0, 0x1000, deep)
	}
	if got, converted := DocumentMarkdown(pptCompound(t, map[string][]byte{"PowerPoint Document": deep}), MediaPPT); converted || got != "" {
		t.Fatalf("deep PPT converted to %q", got)
	}
	many := strings.Repeat("x\r", maxDocumentBlocks+1)
	if got, converted := DocumentMarkdown(pptCompound(t, map[string][]byte{"PowerPoint Document": pptText(1, many, nil)}), MediaPPT); converted || got != "" {
		t.Fatalf("oversized PPT converted to %d characters", len(got))
	}
}
