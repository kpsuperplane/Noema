//go:build !noema_release

package web

import "io/fs"

func packagedAssets() fs.FS { return nil }
