package admintmpl

import "embed"

// Files holds embedded admin HTML templates.
//
//go:embed *.html
var Files embed.FS
