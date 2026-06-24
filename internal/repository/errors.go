package repository

import "errors"

var (
	ErrNoValidExportColumns = errors.New("no valid export columns selected")
	ErrUnknownCategory      = errors.New("unknown category")
	ErrPhoneFilterRequired  = errors.New("category, cnae, or nome_fantasia filter is required")
	ErrInvalidExportDate    = errors.New("invalid date: use YYYY-MM-DD")
)
