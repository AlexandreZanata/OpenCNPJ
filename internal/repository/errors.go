package repository

import "errors"

var (
	ErrNoValidExportColumns = errors.New("no valid export columns selected")
	ErrUnknownCategory      = errors.New("unknown category")
	ErrPhoneFilterRequired  = errors.New("at least one filter is required: category, cnae, nome_fantasia, uf, or city")
	ErrInvalidExportDate    = errors.New("invalid date: use YYYY-MM-DD")
)
