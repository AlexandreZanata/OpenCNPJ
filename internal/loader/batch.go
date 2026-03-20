package loader

import "context"

type BatchInserter interface {
	CopyRows(ctx context.Context, schema, table string, columns []string, rows [][]any) (int64, error)
}

type Batcher struct {
	maxSize int
	rows    [][]any
}

func NewBatcher(maxSize int) *Batcher {
	return &Batcher{
		maxSize: maxSize,
		rows:    make([][]any, 0, maxSize),
	}
}

func (b *Batcher) Add(row []any) (flushed [][]any, shouldFlush bool) {
	b.rows = append(b.rows, row)
	if len(b.rows) >= b.maxSize {
		return b.Flush(), true
	}
	return nil, false
}

func (b *Batcher) Flush() [][]any {
	if len(b.rows) == 0 {
		return nil
	}
	rows := b.rows
	b.rows = make([][]any, 0, b.maxSize)
	return rows
}
