package models

import (
	"encoding/json"
	"testing"
)

func TestNullFloat64MarshalJSON(t *testing.T) {
	valid := NullFloat64{}
	valid.Float64 = 1500.5
	valid.Valid = true
	raw, err := json.Marshal(valid)
	if err != nil {
		t.Fatal(err)
	}
	if string(raw) != "1500.5" {
		t.Fatalf("marshal valid = %s", raw)
	}

	invalid := NullFloat64{}
	raw, err = json.Marshal(invalid)
	if err != nil {
		t.Fatal(err)
	}
	if string(raw) != "null" {
		t.Fatalf("marshal invalid = %s", raw)
	}
}

func TestNullFloat64UnmarshalJSON(t *testing.T) {
	var value NullFloat64
	if err := json.Unmarshal([]byte("2500"), &value); err != nil {
		t.Fatal(err)
	}
	if !value.Valid || value.Float64 != 2500 {
		t.Fatalf("value = %+v", value)
	}

	value = NullFloat64{}
	if err := json.Unmarshal([]byte("null"), &value); err != nil {
		t.Fatal(err)
	}
	if value.Valid {
		t.Fatal("expected invalid null float")
	}
}
