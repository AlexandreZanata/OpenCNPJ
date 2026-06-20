package repository

import "testing"

func TestErrStatsNotReadyMessage(t *testing.T) {
	if ErrStatsNotReady.Error() == "" {
		t.Fatal("ErrStatsNotReady must have a message")
	}
}
