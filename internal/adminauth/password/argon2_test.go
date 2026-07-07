package password

import "testing"

func TestHashAndVerify(t *testing.T) {
	hash, err := Hash("correct-horse-battery")
	if err != nil {
		t.Fatal(err)
	}
	ok, err := Verify(hash, "correct-horse-battery")
	if err != nil || !ok {
		t.Fatalf("verify ok=%v err=%v", ok, err)
	}
	ok, err = Verify(hash, "wrong")
	if err != nil || ok {
		t.Fatalf("wrong password should fail")
	}
}

func TestHashBytesVerifyBytes(t *testing.T) {
	raw, err := HashBytes("secret-pass")
	if err != nil {
		t.Fatal(err)
	}
	ok, err := VerifyBytes(raw, "secret-pass")
	if err != nil || !ok {
		t.Fatalf("bytes verify failed")
	}
}

func TestVerifyBytesEmptyHash(t *testing.T) {
	ok, err := VerifyBytes([]byte{0}, "any")
	if err != nil || ok {
		t.Fatalf("placeholder hash must not verify")
	}
}
