package importer

import (
	"fmt"
	"sync"
)

// DedupeSet tracks keys seen during a single table import group.
type DedupeSet struct {
	mu   sync.Mutex
	keys map[string]struct{}
}

func NewDedupeSet() *DedupeSet {
	return &DedupeSet{keys: make(map[string]struct{})}
}

func (d *DedupeSet) Seen(key string) bool {
	d.mu.Lock()
	defer d.mu.Unlock()
	if _, ok := d.keys[key]; ok {
		return true
	}
	d.keys[key] = struct{}{}
	return false
}

func dedupeKey(table string, row []any) (string, bool) {
	switch table {
	case "empresas", "simples":
		if len(row) < 1 {
			return "", false
		}
		return stringKey(row, 0), true
	case "estabelecimentos":
		if len(row) < 3 {
			return "", false
		}
		return fmt.Sprintf("%v|%v|%v", row[0], row[1], row[2]), true
	case "socios":
		if len(row) < 6 {
			return "", false
		}
		return fmt.Sprintf("%v|%v|%v|%v|%v", row[0], row[2], row[3], row[4], row[5]), true
	default:
		return "", false
	}
}

func stringKey(row []any, idx int) string {
	if idx < 0 || idx >= len(row) {
		return ""
	}
	if s, ok := row[idx].(string); ok {
		return s
	}
	return fmt.Sprintf("%v", row[idx])
}
