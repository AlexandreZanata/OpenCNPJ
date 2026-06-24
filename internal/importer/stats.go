package importer

import (
	"fmt"
	"sync"
	"time"
)

// Stats holds import throughput metrics.
type Stats struct {
	mu        sync.Mutex
	StartedAt time.Time
	TableRows map[string]int64
}

func NewStats() *Stats {
	return &Stats{StartedAt: time.Now(), TableRows: make(map[string]int64)}
}

func (s *Stats) Add(table string, n int64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.TableRows[table] += n
}

func (s *Stats) TotalRows() int64 {
	s.mu.Lock()
	defer s.mu.Unlock()
	var t int64
	for _, n := range s.TableRows {
		t += n
	}
	return t
}

func (s *Stats) Summary() string {
	elapsed := time.Since(s.StartedAt)
	total := s.TotalRows()
	rps := float64(total) / elapsed.Seconds()
	return fmt.Sprintf(
		"rows=%d elapsed=%s rps=%.0f tables=%v",
		total, elapsed.Round(time.Millisecond), rps, s.TableRows,
	)
}
