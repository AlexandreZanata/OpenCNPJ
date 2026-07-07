package saas

import "strings"

// MaskAPIKey redacts a raw API key for safe logging (e.g. ocnjp_live_abcd...).
func MaskAPIKey(raw string) string {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return ""
	}
	if len(raw) <= 16 {
		return raw[:min(len(raw), 8)] + "..."
	}
	return raw[:16] + "..."
}
