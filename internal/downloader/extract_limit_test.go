package downloader

import "testing"

func TestMaxZipMemberBytesAllowsLargeRFBCSVs(t *testing.T) {
	// Historical bug: 512 MiB LimitReader truncated EMPRECSV/ESTABELE members.
	const minSafe = 2 << 30 // 2 GiB
	if maxZipMemberBytes < minSafe {
		t.Fatalf("maxZipMemberBytes=%d want >= %d", maxZipMemberBytes, minSafe)
	}
}

func TestZipMemberErrorSentinels(t *testing.T) {
	if ErrZipMemberTooLarge == nil || ErrZipMemberTruncated == nil {
		t.Fatal("missing zip member error sentinels")
	}
}
