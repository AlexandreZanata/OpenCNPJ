package repository

const (
	// DefaultExportLimit is applied when the client omits limit.
	DefaultExportLimit = 10000
	// MaxCSVExportLimit caps rows per CSV export request (plan 02 bulk export).
	MaxCSVExportLimit = 500000
)

// NormalizeExportLimit applies default and max caps for CSV export.
func NormalizeExportLimit(limit int) int {
	if limit <= 0 {
		return DefaultExportLimit
	}
	if limit > MaxCSVExportLimit {
		return MaxCSVExportLimit
	}
	return limit
}
