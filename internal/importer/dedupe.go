package importer

import (
	"fmt"
	"sync"
)

// DedupeSet tracks keys seen during a single table import.
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

func socioDedupeKey(row []any) string {
	return fmt.Sprintf("%v|%v|%v|%v|%v", row[0], row[2], row[3], row[4], row[5])
}

func empresaDedupeKey(row []any) string {
	if s, ok := row[0].(string); ok {
		return s
	}
	return fmt.Sprintf("%v", row[0])
}
