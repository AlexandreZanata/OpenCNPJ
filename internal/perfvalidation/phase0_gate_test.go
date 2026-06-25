package perfvalidation

import "testing"

func TestPhase0RequiredMetrics(t *testing.T) {
	if len(Phase0RequiredMetrics) != 2 {
		t.Fatalf("metrics = %d, want 2", len(Phase0RequiredMetrics))
	}
	if Phase0RequiredMetrics[0] == "" || Phase0RequiredMetrics[1] == "" {
		t.Fatal("empty metric name")
	}
}
