package downloader

import (
	"fmt"
	"io"
	"os"
	"sync"
)

// ProgressFunc receives file index, total files, filename, bytes written, and expected size (-1 if unknown).
type ProgressFunc func(fileIndex, fileTotal int, filename string, written, totalBytes int64)

// TerminalProgress prints a single updating line to stderr.
type TerminalProgress struct {
	mu sync.Mutex
}

// NewTerminalProgress creates a stderr progress renderer.
func NewTerminalProgress() *TerminalProgress {
	return &TerminalProgress{}
}

// Callback returns a ProgressFunc suitable for Options.OnProgress.
func (t *TerminalProgress) Callback() ProgressFunc {
	return func(fileIndex, fileTotal int, filename string, written, totalBytes int64) {
		t.mu.Lock()
		defer t.mu.Unlock()
		line := formatProgressLine(fileIndex, fileTotal, filename, written, totalBytes)
		fmt.Fprintf(os.Stderr, "\r%s", line)
	}
}

// Done clears the progress line and prints a newline.
func (t *TerminalProgress) Done() {
	t.mu.Lock()
	defer t.mu.Unlock()
	fmt.Fprint(os.Stderr, "\r\033[K\n")
}

func formatProgressLine(fileIndex, fileTotal int, filename string, written, totalBytes int64) string {
	filePct := float64(fileIndex) / float64(fileTotal) * 100
	if fileIndex > 0 && fileIndex <= fileTotal {
		filePct = float64(fileIndex-1) / float64(fileTotal) * 100
	}

	var bytePct float64
	byteLabel := humanBytes(written)
	if totalBytes > 0 {
		bytePct = float64(written) / float64(totalBytes) * 100
		byteLabel = fmt.Sprintf("%s / %s", humanBytes(written), humanBytes(totalBytes))
	}

	overall := filePct
	if totalBytes > 0 {
		fileWeight := 100.0 / float64(fileTotal)
		overall = filePct + (bytePct * fileWeight / 100)
	}

	return fmt.Sprintf(
		"[%d/%d] %5.1f%%  %s  (%s)",
		fileIndex, fileTotal, overall, trimFilename(filename, 32), byteLabel,
	)
}

func trimFilename(name string, max int) string {
	if len(name) <= max {
		return name
	}
	return "…" + name[len(name)-max+1:]
}

func humanBytes(n int64) string {
	const unit = 1024
	if n < unit {
		return fmt.Sprintf("%d B", n)
	}
	div, exp := int64(unit), 0
	for v := n / unit; v >= unit; v /= unit {
		div *= unit
		exp++
	}
	return fmt.Sprintf("%.1f %cB", float64(n)/float64(div), "KMGTPE"[exp])
}

type countingWriter struct {
	w      io.Writer
	written int64
	onWrite func(written int64)
}

func (c *countingWriter) Write(p []byte) (int, error) {
	n, err := c.w.Write(p)
	if n > 0 {
		c.written += int64(n)
		if c.onWrite != nil {
			c.onWrite(c.written)
		}
	}
	return n, err
}
