//go:build noema_release

package web

import (
	"embed"
	"io/fs"
)

//go:embed release-assets/*
var releaseAssets embed.FS

func packagedAssets() fs.FS {
	assets, _ := fs.Sub(releaseAssets, "release-assets")
	return assets
}
