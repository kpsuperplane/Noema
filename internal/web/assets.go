// Package web serves the browser application built by Vite.
package web

import (
	"io/fs"
	"net/http"
	"os"
	"path/filepath"
	"strings"
)

const defaultAssetDirectory = "crates/noema-server/target/web-assets"

var spaExcludedPrefixes = [...]string{
	"/assets",
	"/api",
	"/graphql",
	"/auth",
	"/oauth",
	"/__noema",
	"/mcp/oauth",
	"/provider/oauth",
	"/adapter/oauth",
	"/artifacts",
}

type assetHandler struct {
	assets fs.FS
}

// NewAssetHandler serves the current Vite build and browser routes.
func NewAssetHandler() http.Handler {
	return assetHandler{assets: assetFileSystem()}
}

func assetFileSystem() fs.FS {
	if assets := packagedAssets(); assets != nil {
		return assets
	}
	if directory := os.Getenv("NOEMA_DEV_ASSET_DIR"); directory != "" {
		return os.DirFS(directory)
	}
	return os.DirFS(filepath.FromSlash(defaultAssetDirectory))
}

func (h assetHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.NotFound(w, r)
		return
	}
	name, contentType, ok := assetRequest(r.URL.Path)
	if !ok {
		http.NotFound(w, r)
		return
	}
	body, err := fs.ReadFile(h.assets, name)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	w.Header().Set("Content-Type", contentType)
	w.Header().Set("Cache-Control", cacheControl(name))
	if name == "sw.js" {
		w.Header().Set("Service-Worker-Allowed", "/")
	}
	_, _ = w.Write(body)
}

func assetRequest(path string) (name string, contentType string, ok bool) {
	if name, ok = staticAssetName(path); ok {
		contentType, ok = assetContentType(name)
		return name, contentType, ok
	}
	if IsSPAPath(path) {
		return "index.html", "text/html; charset=utf-8", true
	}
	return "", "", false
}

func staticAssetName(path string) (string, bool) {
	if path == "/favicon.ico" {
		return "favicon.ico", true
	}
	name, ok := strings.CutPrefix(path, "/assets/")
	if !ok || name == "" || name == "graphiql.html" ||
		strings.ContainsAny(name, "/\\") || strings.Contains(name, "..") {
		return "", false
	}
	return name, true
}

// IsSPAPath reports whether a path belongs to the browser application.
func IsSPAPath(path string) bool {
	if !strings.HasPrefix(path, "/") {
		return false
	}
	for _, prefix := range spaExcludedPrefixes {
		if path == prefix || strings.HasPrefix(path, prefix+"/") {
			return false
		}
	}
	segment := path[strings.LastIndexByte(path, '/')+1:]
	return !strings.Contains(segment, ".")
}

func assetContentType(name string) (string, bool) {
	extension := name[strings.LastIndexByte(name, '.')+1:]
	switch extension {
	case "js":
		return "application/javascript; charset=utf-8", true
	case "css":
		return "text/css; charset=utf-8", true
	case "svg":
		return "image/svg+xml; charset=utf-8", true
	case "png":
		return "image/png", true
	case "ico":
		return "image/x-icon", true
	case "ttf":
		return "font/ttf", true
	case "webmanifest":
		return "application/manifest+json; charset=utf-8", true
	case "html":
		return "text/html; charset=utf-8", true
	default:
		return "", false
	}
}

func cacheControl(name string) string {
	extension := name[strings.LastIndexByte(name, '.')+1:]
	if name != "sw.js" && (extension == "js" || extension == "css" || extension == "ttf") {
		return "public, max-age=31536000, immutable"
	}
	return "no-cache"
}
