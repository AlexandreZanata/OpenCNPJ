package repository

import "errors"

var (
	ErrEmptyCursor          = errors.New("empty cursor")
	ErrInvalidCursorSegment = errors.New("invalid cursor segment")
	ErrCursorMissingScore   = errors.New("cursor missing score")
	ErrCursorMissingCNPJ    = errors.New("cursor missing cnpj")
	ErrCursorMissingID      = errors.New("cursor missing id")
)
