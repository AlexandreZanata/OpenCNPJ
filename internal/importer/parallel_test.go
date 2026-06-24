package importer

import (
	"context"
	"sync/atomic"
	"testing"
)

func TestParallelFiles(t *testing.T) {
	var count atomic.Int32
	paths := []string{"a", "b", "c", "d"}
	err := parallelFiles(context.Background(), paths, 2, func(_ context.Context, _ string) error {
		count.Add(1)
		return nil
	})
	if err != nil || count.Load() != 4 {
		t.Fatalf("got count=%d err=%v", count.Load(), err)
	}
}
