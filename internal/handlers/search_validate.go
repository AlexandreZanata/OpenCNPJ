package handlers

import (
	"fmt"
	"unicode/utf8"
)

const minFuzzySearchRunes = 3

func validateFuzzySearchTerm(field, value string) error {
	if value == "" {
		return nil
	}
	if utf8.RuneCountInString(value) < minFuzzySearchRunes {
		return fmt.Errorf("%s must be at least %d characters", field, minFuzzySearchRunes)
	}
	return nil
}
