package totp

import "testing"

func TestGenerateAndValidate(t *testing.T) {
	svc := NewService("OpenCNPJ-Admin")
	secret, url, err := svc.Generate("admin@test.local")
	if err != nil || secret == "" || url == "" {
		t.Fatalf("generate failed: secret=%q url=%q err=%v", secret, url, err)
	}
	code, err := svc.CurrentCode(secret)
	if err != nil {
		t.Fatal(err)
	}
	if !svc.Validate(secret, code) {
		t.Fatal("expected valid code")
	}
	if svc.Validate(secret, "000000") {
		t.Fatal("invalid code should fail")
	}
}
