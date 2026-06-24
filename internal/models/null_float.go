package models

import (
	"database/sql"
	"encoding/json"
	"errors"
)

// NullFloat64 scans sql.NullFloat64 and marshals as JSON number or null.
type NullFloat64 struct {
	sql.NullFloat64
}

var errNullFloat64NilReceiver = errors.New("NullFloat64: nil receiver")

func (n *NullFloat64) Scan(value any) error {
	if n == nil {
		return errNullFloat64NilReceiver
	}
	return n.NullFloat64.Scan(value)
}

func (n NullFloat64) MarshalJSON() ([]byte, error) {
	if !n.Valid {
		return []byte("null"), nil
	}
	return json.Marshal(n.Float64)
}

func (n *NullFloat64) UnmarshalJSON(data []byte) error {
	if string(data) == "null" {
		n.Valid = false
		return nil
	}
	var value float64
	if err := json.Unmarshal(data, &value); err != nil {
		return err
	}
	n.Float64 = value
	n.Valid = true
	return nil
}
