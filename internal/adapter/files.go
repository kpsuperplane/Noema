package adapter

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"sort"
	"strings"

	"github.com/kpsuperplane/noema/internal/home"
)

const (
	definitionLimit = 1024
	connectionLimit = 1024
	connectionBytes = 128 << 10
)

type provenance struct {
	SourceReference     string   `json:"source_reference"`
	Replaces            []string `json:"replaces_semantic_digests,omitempty"`
	AffectedConnections []string `json:"affected_connection_ids,omitempty"`
}

type fileAuthority struct{ root *os.Root }

func newFileAuthority(root *os.Root) (*fileAuthority, error) {
	if root == nil {
		return nil, errors.New("adapter home is unavailable")
	}
	for _, path := range []string{"adapters", "adapters/definitions", "adapters/connections", "adapters/cursors", "adapters/quarantine", "adapters/quarantine/definitions", "adapters/quarantine/connections"} {
		if err := root.MkdirAll(path, 0o700); err != nil {
			return nil, errors.New("adapter home could not be prepared")
		}
	}
	authority := &fileAuthority{root: root}
	if err := authority.recoverStages("adapters/definitions"); err != nil {
		return nil, err
	}
	if err := authority.recoverStages("adapters/connections"); err != nil {
		return nil, err
	}
	if err := authority.recoverConnectionReplacements(); err != nil {
		return nil, err
	}
	return authority, nil
}

func (f *fileAuthority) recoverConnectionReplacements() error {
	entries, err := readBoundedEntries(f.root, "adapters/connections", connectionLimit)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		if !entry.IsDir() || !validConnectionID(entry.Name()) {
			continue
		}
		directory, openErr := f.root.OpenRoot("adapters/connections/" + entry.Name())
		if openErr != nil {
			continue
		}
		children, readErr := readBoundedEntries(directory, ".", 64)
		changed := false
		if readErr == nil {
			for _, child := range children {
				name := child.Name()
				if strings.HasPrefix(name, ".connection-") && strings.HasSuffix(name, ".json") && child.Type().IsRegular() {
					if removeErr := directory.Remove(name); removeErr != nil {
						_ = directory.Close()
						return errors.New("adapter connection recovery failed")
					}
					changed = true
				}
			}
		}
		if changed {
			readErr = home.SyncRootDirectory(directory, ".")
		}
		_ = directory.Close()
		if readErr != nil {
			return errors.New("adapter connection recovery failed")
		}
	}
	return nil
}

func (f *fileAuthority) recoverStages(path string) error {
	directory, err := f.root.Open(path)
	if err != nil {
		return errors.New("adapter staging directory is unavailable")
	}
	defer directory.Close()
	entries, err := directory.ReadDir(definitionLimit + connectionLimit + 1)
	if err != nil && !errors.Is(err, io.EOF) {
		return errors.New("adapter staging directory could not be read")
	}
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".staging-") {
			if err := f.root.RemoveAll(path + "/" + entry.Name()); err != nil {
				return errors.New("adapter staging recovery failed")
			}
		}
	}
	return nil
}

func (f *fileAuthority) installDefinition(manifest Manifest, sourceReference string, replaces, affected []string) (Definition, error) {
	definition, err := Compile(manifest)
	if err != nil {
		return Definition{}, err
	}
	manifestRaw, err := json.Marshal(manifest)
	if err != nil || len(manifestRaw) > manifestLimit {
		return Definition{}, errors.New("adapter manifest is invalid")
	}
	sort.Strings(replaces)
	sort.Strings(affected)
	metadataRaw, err := json.Marshal(provenance{SourceReference: sourceReference, Replaces: replaces, AffectedConnections: affected})
	if err != nil || len(metadataRaw) > 64<<10 {
		return Definition{}, errors.New("adapter provenance is invalid")
	}
	path := "adapters/definitions/" + definition.SemanticDigest
	if _, err := f.root.Lstat(path); err == nil {
		existing, loadErr := f.loadDefinition(definition.SemanticDigest)
		if loadErr != nil {
			return Definition{}, loadErr
		}
		if existing.SourceReference != sourceReference || strings.Join(existing.Replaces, "\x00") != strings.Join(replaces, "\x00") || strings.Join(existing.AffectedConnections, "\x00") != strings.Join(affected, "\x00") {
			return Definition{}, errors.New("adapter definition publication conflicts")
		}
		return existing, nil
	}
	if err := f.installDirectory("adapters/definitions", definition.SemanticDigest, map[string][]byte{"manifest.json": manifestRaw, "provenance.json": metadataRaw}); err != nil {
		return Definition{}, err
	}
	return f.loadDefinition(definition.SemanticDigest)
}

func (f *fileAuthority) installConnection(connection Connection) (Connection, error) {
	raw, err := json.Marshal(connection)
	if err != nil || len(raw) > connectionBytes {
		return Connection{}, errors.New("adapter connection is invalid")
	}
	if err := f.installDirectory("adapters/connections", connection.ConnectionID, map[string][]byte{"connection.json": raw}); err != nil {
		return Connection{}, err
	}
	return f.loadConnection(connection.ConnectionID)
}

func (f *fileAuthority) replaceConnection(connection Connection) (Connection, error) {
	raw, err := json.Marshal(connection)
	if err != nil || len(raw) > connectionBytes {
		return Connection{}, errors.New("adapter connection is invalid")
	}
	directory, err := f.root.OpenRoot("adapters/connections/" + connection.ConnectionID)
	if err != nil {
		return Connection{}, errors.New("adapter connection is unavailable")
	}
	defer directory.Close()
	temporary := ".connection-" + randomHex() + ".json"
	if err := writeNewFile(directory, temporary, raw); err != nil {
		return Connection{}, err
	}
	if err := home.ReplaceRootFile(directory, temporary, "connection.json"); err != nil {
		_ = directory.Remove(temporary)
		return Connection{}, errors.New("adapter connection publication failed")
	}
	if err := home.SyncRootDirectory(directory, "."); err != nil {
		return Connection{}, errors.New("adapter connection durability failed")
	}
	return f.loadConnection(connection.ConnectionID)
}

func (f *fileAuthority) installDirectory(parent, name string, files map[string][]byte) error {
	temporary := ".staging-" + randomHex()
	stagePath := parent + "/" + temporary
	if err := f.root.Mkdir(stagePath, 0o700); err != nil {
		return errors.New("adapter staging directory could not be created")
	}
	stage, err := f.root.OpenRoot(stagePath)
	if err != nil {
		_ = f.root.RemoveAll(stagePath)
		return errors.New("adapter staging directory is unavailable")
	}
	ok := false
	defer func() {
		_ = stage.Close()
		if !ok {
			_ = f.root.RemoveAll(stagePath)
		}
	}()
	names := make([]string, 0, len(files))
	for value := range files {
		names = append(names, value)
	}
	sort.Strings(names)
	for _, value := range names {
		if err := writeNewFile(stage, value, files[value]); err != nil {
			return err
		}
	}
	if err := home.SyncRootDirectory(stage, "."); err != nil {
		return errors.New("adapter staging durability failed")
	}
	if err := stage.Close(); err != nil {
		return errors.New("adapter staging directory could not be closed")
	}
	if err := f.root.Rename(stagePath, parent+"/"+name); err != nil {
		if _, inspect := f.root.Lstat(parent + "/" + name); inspect == nil {
			return nil
		}
		return errors.New("adapter object publication failed")
	}
	ok = true
	return home.SyncRootDirectory(f.root, parent)
}

func writeNewFile(root *os.Root, name string, raw []byte) error {
	file, err := root.OpenFile(name, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return errors.New("adapter file could not be created")
	}
	if written, writeErr := file.Write(raw); writeErr != nil || written != len(raw) {
		_ = file.Close()
		return errors.New("adapter file could not be written")
	}
	if err := file.Sync(); err != nil {
		_ = file.Close()
		return errors.New("adapter file could not be synced")
	}
	if err := file.Close(); err != nil {
		return errors.New("adapter file could not be closed")
	}
	return nil
}

func (f *fileAuthority) loadDefinition(digest string) (Definition, error) {
	if !validDigest(digest) {
		return Definition{}, errors.New("adapter definition is unavailable")
	}
	directory, err := f.root.OpenRoot("adapters/definitions/" + digest)
	if err != nil {
		return Definition{}, errors.New("adapter definition is unavailable")
	}
	defer directory.Close()
	if err := exactFiles(directory, []string{"manifest.json", "provenance.json"}); err != nil {
		return Definition{}, err
	}
	manifestRaw, err := readRegular(directory, "manifest.json", manifestLimit)
	if err != nil {
		return Definition{}, err
	}
	definition, err := CompileJSON(manifestRaw)
	if err != nil || definition.SemanticDigest != digest {
		return Definition{}, errors.New("adapter definition content changed")
	}
	metadataRaw, err := readRegular(directory, "provenance.json", 64<<10)
	if err != nil {
		return Definition{}, err
	}
	var metadata provenance
	if decodeExactJSON(metadataRaw, &metadata) != nil || validateSourceReference(metadata.SourceReference) != nil {
		return Definition{}, errors.New("adapter definition provenance is invalid")
	}
	for _, replaced := range metadata.Replaces {
		if !validDigest(replaced) || replaced == digest {
			return Definition{}, errors.New("adapter definition lineage is invalid")
		}
	}
	for index, id := range metadata.AffectedConnections {
		if !validConnectionID(id) || index > 0 && metadata.AffectedConnections[index-1] >= id {
			return Definition{}, errors.New("adapter definition transition is invalid")
		}
	}
	definition.SourceReference = metadata.SourceReference
	definition.Replaces = append([]string(nil), metadata.Replaces...)
	definition.AffectedConnections = append([]string(nil), metadata.AffectedConnections...)
	return definition, nil
}

func (f *fileAuthority) definitions() ([]Definition, error) {
	entries, err := readBoundedEntries(f.root, "adapters/definitions", definitionLimit)
	if err != nil {
		return nil, err
	}
	result := make([]Definition, 0, len(entries))
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".") {
			continue
		}
		if entry.Type()&os.ModeSymlink != 0 || !entry.IsDir() || !validDigest(entry.Name()) {
			continue
		}
		value, loadErr := f.loadDefinition(entry.Name())
		if loadErr == nil {
			result = append(result, value)
		}
	}
	sort.Slice(result, func(i, j int) bool { return result[i].SemanticDigest < result[j].SemanticDigest })
	for i := range result {
		for j := range result {
			for _, replaced := range result[j].Replaces {
				if replaced == result[i].SemanticDigest {
					result[i].Superseded = true
				}
			}
		}
		if result[i].Manifest.Reviewed {
			continue
		}
		for j := range result {
			if result[j].Manifest.Reviewed && result[j].Manifest.DefinitionID == result[i].Manifest.DefinitionID && result[j].Manifest.DefinitionRevision == result[i].Manifest.DefinitionRevision {
				result[i].Superseded = true
			}
		}
	}
	return result, nil
}

func (f *fileAuthority) loadConnection(id string) (Connection, error) {
	if !validConnectionID(id) {
		return Connection{}, errors.New("adapter connection is unavailable")
	}
	directory, err := f.root.OpenRoot("adapters/connections/" + id)
	if err != nil {
		return Connection{}, errors.New("adapter connection is unavailable")
	}
	defer directory.Close()
	if err := exactFiles(directory, []string{"connection.json"}); err != nil {
		return Connection{}, err
	}
	raw, err := readRegular(directory, "connection.json", connectionBytes)
	if err != nil {
		return Connection{}, err
	}
	var value Connection
	if decodeExactJSON(raw, &value) != nil || validateConnection(value) != nil {
		return Connection{}, errors.New("adapter connection is invalid")
	}
	return value, nil
}

func (f *fileAuthority) connections() ([]Connection, error) {
	entries, err := readBoundedEntries(f.root, "adapters/connections", connectionLimit)
	if err != nil {
		return nil, err
	}
	result := make([]Connection, 0, len(entries))
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".") {
			continue
		}
		if entry.Type()&os.ModeSymlink != 0 || !entry.IsDir() || !validConnectionID(entry.Name()) {
			continue
		}
		value, loadErr := f.loadConnection(entry.Name())
		if loadErr == nil {
			result = append(result, value)
		}
	}
	sort.Slice(result, func(i, j int) bool { return result[i].ConnectionID < result[j].ConnectionID })
	return result, nil
}

func validateConnection(value Connection) error {
	if value.SchemaVersion != 1 || !validConnectionID(value.ConnectionID) || !validID(value.ConnectionSlug) || !validDigest(value.SemanticDigest) || value.ConnectionRevision < 1 || value.PolicyRevision < 1 || (value.Status != "active" && value.Status != "suspended") {
		return errors.New("adapter connection is invalid")
	}
	if value.DataSharingPolicy != "" && value.DataSharingPolicy != "allow_automatically" && value.DataSharingPolicy != "review_every_call" {
		return errors.New("adapter connection policy is invalid")
	}
	if value.UnsafeActionPolicy != "" && value.UnsafeActionPolicy != "always_ask" && value.UnsafeActionPolicy != "reviewer_may_approve" && value.UnsafeActionPolicy != "never_ask" {
		return errors.New("adapter connection policy is invalid")
	}
	seen := map[string]bool{}
	for _, id := range value.AllowedOperations {
		if !validID(id) || seen[id] {
			return errors.New("adapter connection operations are invalid")
		}
		seen[id] = true
	}
	return nil
}

func (f *fileAuthority) quarantine(kind, id string) error {
	if kind != "definitions" && kind != "connections" {
		return errors.New("adapter quarantine kind is invalid")
	}
	if err := f.root.Rename("adapters/"+kind+"/"+id, "adapters/quarantine/"+kind+"/"+id+"-"+randomHex()); err != nil {
		return err
	}
	if err := home.SyncRootDirectory(f.root, "adapters/"+kind); err != nil {
		return err
	}
	return home.SyncRootDirectory(f.root, "adapters/quarantine/"+kind)
}

func exactFiles(root *os.Root, expected []string) error {
	entries, err := readBoundedEntries(root, ".", len(expected))
	if err != nil {
		return err
	}
	if len(entries) != len(expected) {
		return errors.New("adapter object has unexpected files")
	}
	sort.Strings(expected)
	for i, entry := range entries {
		if entry.Name() != expected[i] || entry.Type()&os.ModeSymlink != 0 || !entry.Type().IsRegular() {
			return errors.New("adapter object has unsafe files")
		}
	}
	return nil
}
func readBoundedEntries(root *os.Root, path string, limit int) ([]fs.DirEntry, error) {
	file, err := root.Open(path)
	if err != nil {
		return nil, errors.New("adapter directory is unavailable")
	}
	defer file.Close()
	entries, err := file.ReadDir(limit + 1)
	if err != nil && !errors.Is(err, io.EOF) {
		return nil, errors.New("adapter directory could not be read")
	}
	if len(entries) > limit {
		return nil, errors.New("adapter directory has too many entries")
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Name() < entries[j].Name() })
	return entries, nil
}
func readRegular(root *os.Root, name string, limit int64) ([]byte, error) {
	info, err := root.Lstat(name)
	if err != nil || !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 || info.Size() > limit {
		return nil, errors.New("adapter file is unsafe")
	}
	file, err := root.Open(name)
	if err != nil {
		return nil, errors.New("adapter file is unavailable")
	}
	defer file.Close()
	raw, err := io.ReadAll(io.LimitReader(file, limit+1))
	if err != nil || int64(len(raw)) > limit {
		return nil, errors.New("adapter file is too large")
	}
	return raw, nil
}
func randomHex() string {
	var value [16]byte
	if _, err := rand.Read(value[:]); err != nil {
		panic(fmt.Sprintf("random source failed: %v", err))
	}
	return hex.EncodeToString(value[:])
}
func validDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, r := range value {
		if !(r >= '0' && r <= '9' || r >= 'a' && r <= 'f') {
			return false
		}
	}
	return true
}
func validConnectionID(value string) bool {
	if len(value) != 32 {
		return false
	}
	for _, r := range value {
		if !(r >= '0' && r <= '9' || r >= 'a' && r <= 'f') {
			return false
		}
	}
	return true
}
