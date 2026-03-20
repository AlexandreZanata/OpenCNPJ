package parser

import "fmt"

type InvalidColumnCountError struct {
	Entity   string
	Expected int
	Got      int
}

func (e InvalidColumnCountError) Error() string {
	return fmt.Sprintf("%s invalid column count expected=%d got=%d", e.Entity, e.Expected, e.Got)
}

type InvalidFieldError struct {
	Field  string
	Value  string
	Reason string
}

func (e InvalidFieldError) Error() string {
	return fmt.Sprintf("invalid field=%s value=%q reason=%s", e.Field, e.Value, e.Reason)
}
