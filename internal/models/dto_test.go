package models

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestSearchResponseEmptyDataEncodesAsArray(t *testing.T) {
	resp := SearchResponse{
		Data:   make([]Empresa, 0),
		Total:  0,
		Limit:  20,
		Offset: 0,
	}
	raw, err := json.Marshal(resp)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	body := string(raw)
	if strings.Contains(body, `"data":null`) {
		t.Fatalf("expected empty array, got: %s", body)
	}
	if !strings.Contains(body, `"data":[]`) {
		t.Fatalf("expected data:[], got: %s", body)
	}
}
