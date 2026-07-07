package adminstatic

import "embed"

// Files holds embedded admin static assets.
//
//go:embed admin.css
var Files embed.FS
