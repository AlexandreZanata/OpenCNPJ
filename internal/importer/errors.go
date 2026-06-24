package importer

import "errors"

var (
	ErrNoEmpresasFiles      = errors.New("no EMPRECSV files in data directory")
	ErrInvalidSamplePercent = errors.New("sample percent must be > 0")
	ErrReferenceRowColumns  = errors.New("reference row needs 2 columns")
	ErrCNAERowColumns       = errors.New("cnae row needs 2 columns")
)
