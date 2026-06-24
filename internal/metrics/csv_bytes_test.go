package metrics

import "testing"

func TestCSVRecordBytes(t *testing.T) {
	got := CSVRecordBytes([]string{"41273590", "ACME LTDA", "4014"})
	if got == 0 {
		t.Fatal("expected positive byte estimate")
	}
	if got < 20 {
		t.Fatalf("estimate too small: %d", got)
	}
}

func TestCSVRecordBytes_empty(t *testing.T) {
	if CSVRecordBytes(nil) != 0 {
		t.Fatal("expected zero for empty record")
	}
}
