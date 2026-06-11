package downloader

import "testing"

func TestParseHrefs(t *testing.T) {
	body := []byte(`<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response><d:href>/public.php/webdav/</d:href></d:response>
  <d:response><d:href>/public.php/webdav/2026-05/</d:href></d:response>
  <d:response><d:href>/public.php/webdav/Cnaes.zip</d:href></d:response>
</d:multistatus>`)

	hrefs := parseHrefs(body)
	if len(hrefs) != 3 {
		t.Fatalf("expected 3 hrefs, got %d: %v", len(hrefs), hrefs)
	}
}

func TestExtractZipName(t *testing.T) {
	if got := extractZipName("/public.php/webdav/2026-05/Cnaes.zip"); got != "Cnaes.zip" {
		t.Fatalf("got %q", got)
	}
}
