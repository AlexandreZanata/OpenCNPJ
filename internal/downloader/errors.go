package downloader

import "errors"

var (
	ErrEmptyMonthList       = errors.New("empty month list")
	ErrNoMonthlyDirs        = errors.New("no monthly directories found in Receita Federal repository")
	ErrNoZipFiles           = errors.New("no zip files found in month folder")
	ErrCreateOutputDir      = errors.New("create output directory")
	ErrOpenZip              = errors.New("open zip")
	ErrMonthNotAvailable    = errors.New("month not available")
	ErrUnexpectedHTTPStatus = errors.New("unexpected HTTP status")
	ErrZipMemberTooLarge    = errors.New("zip member exceeds max size")
	ErrZipMemberTruncated   = errors.New("zip member size mismatch (truncated extract)")
)
