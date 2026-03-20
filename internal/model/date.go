package model

import (
	"database/sql/driver"
	"fmt"
	"time"
)

// Date wraps time.Time to parse Receita's YYYYMMDD format.
type Date struct {
	time.Time
}

// ParseDate parses YYYYMMDD and maps 00000000/empty to nil.
func ParseDate(raw string) (*Date, error) {
	if raw == "" || raw == "00000000" {
		return nil, nil
	}

	if len(raw) != 8 {
		return nil, DateParseError{Value: raw, Reason: "invalid length"}
	}

	t, err := time.Parse("20060102", raw)
	if err != nil {
		return nil, DateParseError{Value: raw, Reason: err.Error()}
	}

	return &Date{Time: t.UTC()}, nil
}

func (d *Date) Value() (driver.Value, error) {
	if d == nil {
		return nil, nil
	}
	return d.Time, nil
}

func (d *Date) String() string {
	if d == nil {
		return ""
	}
	return d.Time.Format("2006-01-02")
}

type DateParseError struct {
	Value  string
	Reason string
}

func (e DateParseError) Error() string {
	return fmt.Sprintf("date parse error value=%q reason=%s", e.Value, e.Reason)
}
