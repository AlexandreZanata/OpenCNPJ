package apidocs

import "testing"

func TestDefaultPublicDocsURL(t *testing.T) {
	if DefaultPublicDocsURL == "" {
		t.Fatal("empty docs url")
	}
}
