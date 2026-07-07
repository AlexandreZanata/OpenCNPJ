package apidocs

import "embed"

// Static files for /docs (Redoc UI + OpenAPI spec).
//
//go:embed static/*
var static embed.FS
