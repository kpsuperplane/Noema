package graphql

import (
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strings"
	"testing"

	gqlparser "github.com/vektah/gqlparser/v2"
	"github.com/vektah/gqlparser/v2/ast"
	"github.com/vektah/gqlparser/v2/validator/rules"
)

func TestCheckedInClientOperationsMatchSchema(t *testing.T) {
	_, testFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("find test file")
	}
	repository := filepath.Clean(filepath.Join(filepath.Dir(testFile), "..", ".."))
	schemaPath := filepath.Join(repository, "graphql", "schema.graphql")
	schemaSource, err := os.ReadFile(schemaPath)
	if err != nil {
		t.Fatalf("read schema: %v", err)
	}
	schema, err := gqlparser.LoadSchema(&ast.Source{Name: schemaPath, Input: string(schemaSource)})
	if err != nil {
		t.Fatalf("load schema: %v", err)
	}

	gqlLiteral := regexp.MustCompile("(?s)\\bgql\\s*`([^`]*)`")
	interpolation := regexp.MustCompile(`\$\{[^}]+\}`)
	readWeb := func() string {
		t.Helper()
		root := filepath.Join(repository, "apps", "web", "src", "graphql")
		var paths []string
		err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			if !entry.IsDir() && (strings.HasSuffix(path, ".ts") || strings.HasSuffix(path, ".tsx")) {
				paths = append(paths, path)
			}
			return nil
		})
		if err != nil {
			t.Fatalf("scan web operations: %v", err)
		}
		sort.Strings(paths)
		var document strings.Builder
		for _, path := range paths {
			source, readErr := os.ReadFile(path)
			if readErr != nil {
				t.Fatalf("read %s: %v", path, readErr)
			}
			for _, match := range gqlLiteral.FindAllSubmatch(source, -1) {
				document.WriteString("\n# ")
				document.WriteString(filepath.ToSlash(path))
				document.WriteByte('\n')
				document.Write(interpolation.ReplaceAll(match[1], nil))
			}
		}
		return document.String()
	}
	readIOS := func() string {
		t.Helper()
		root := filepath.Join(repository, "apps", "ios", "Noema", "Operations")
		var paths []string
		err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			if !entry.IsDir() && strings.HasSuffix(path, ".graphql") {
				paths = append(paths, path)
			}
			return nil
		})
		if err != nil {
			t.Fatalf("scan iOS operations: %v", err)
		}
		sort.Strings(paths)
		var document strings.Builder
		for _, path := range paths {
			source, readErr := os.ReadFile(path)
			if readErr != nil {
				t.Fatalf("read %s: %v", path, readErr)
			}
			document.WriteString("\n# ")
			document.WriteString(filepath.ToSlash(path))
			document.WriteByte('\n')
			document.Write(source)
		}
		return document.String()
	}
	readGeneratedIOS := func() string {
		t.Helper()
		root := filepath.Join(repository, "apps", "ios", "Noema", "Generated", "Sources")
		rawGraphQL := regexp.MustCompile(`(?s)#"(.*?)"#`)
		var paths []string
		for _, directory := range []string{"Operations", "Fragments"} {
			err := filepath.WalkDir(filepath.Join(root, directory), func(path string, entry fs.DirEntry, walkErr error) error {
				if walkErr != nil {
					return walkErr
				}
				if !entry.IsDir() && strings.HasSuffix(path, ".graphql.swift") {
					paths = append(paths, path)
				}
				return nil
			})
			if err != nil {
				t.Fatalf("scan generated iOS operations: %v", err)
			}
		}
		sort.Strings(paths)
		var document strings.Builder
		for _, path := range paths {
			source, readErr := os.ReadFile(path)
			if readErr != nil {
				t.Fatalf("read %s: %v", path, readErr)
			}
			for _, match := range rawGraphQL.FindAllSubmatch(source, -1) {
				definition := strings.TrimSpace(string(match[1]))
				if !strings.HasPrefix(definition, "query ") &&
					!strings.HasPrefix(definition, "mutation ") &&
					!strings.HasPrefix(definition, "subscription ") &&
					!strings.HasPrefix(definition, "fragment ") {
					continue
				}
				document.WriteString("\n# ")
				document.WriteString(filepath.ToSlash(path))
				document.WriteByte('\n')
				document.WriteString(definition)
			}
		}
		return document.String()
	}

	clients := []struct {
		name   string
		source string
	}{
		{name: "web", source: readWeb()},
		{name: "ios", source: readIOS()},
		{name: "ios-generated", source: readGeneratedIOS()},
	}
	for _, client := range clients {
		t.Run(client.name, func(t *testing.T) {
			document, queryErrors := gqlparser.LoadQueryWithRules(
				schema,
				client.source,
				rules.NewDefaultRules(),
			)
			if len(queryErrors) != 0 {
				t.Fatalf("validate %s operations: %v", client.name, queryErrors)
			}
			if len(document.Operations) == 0 {
				t.Fatalf("no %s operations found", client.name)
			}
		})
	}
}
