package downloader_test

import (
	"testing"

	"busca-cnpj-2026/internal/downloader"
)

func TestZipMemberErrorsExported(t *testing.T) {
	if downloader.ErrZipMemberTooLarge == nil || downloader.ErrZipMemberTruncated == nil {
		t.Fatal("expected exported zip member errors")
	}
}
