package cipher

import "testing"

func testKey() []byte {
	return []byte("01234567890123456789012345678901")
}

func TestAESGCMRoundTrip(t *testing.T) {
	c, err := NewAESGCM(testKey())
	if err != nil {
		t.Fatal(err)
	}
	enc, err := c.EncryptString("JBSWY3DPEHPK3PXP")
	if err != nil {
		t.Fatal(err)
	}
	got, err := c.DecryptString(enc)
	if err != nil || got != "JBSWY3DPEHPK3PXP" {
		t.Fatalf("round trip failed: %q err=%v", got, err)
	}
}

func TestAESGCMRejectShortKey(t *testing.T) {
	if _, err := NewAESGCM([]byte("short")); err == nil {
		t.Fatal("expected error for short key")
	}
}
