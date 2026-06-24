package exportcategory

import "strings"

// SearchPresets returns catalog entries matching query on key, label, or description.
func SearchPresets(query string, limit int) []Category {
	if limit <= 0 {
		limit = 10
	}
	q := strings.ToLower(strings.TrimSpace(query))
	if q == "" {
		return List()[:minInt(len(catalog), limit)]
	}

	matches := make([]Category, 0, limit)
	for _, item := range catalog {
		if matchesPreset(item, q) {
			matches = append(matches, item)
			if len(matches) >= limit {
				break
			}
		}
	}
	return matches
}

func matchesPreset(item Category, query string) bool {
	if strings.Contains(strings.ToLower(item.Key), query) {
		return true
	}
	if strings.Contains(strings.ToLower(item.Label), query) {
		return true
	}
	if strings.Contains(strings.ToLower(item.Description), query) {
		return true
	}
	for _, code := range item.CNAECodes {
		if strings.HasPrefix(code, query) {
			return true
		}
	}
	for _, keyword := range item.Keywords {
		if strings.Contains(keyword, query) {
			return true
		}
	}
	return false
}

func minInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}
