package handlers

import (
	"errors"
	"fmt"
	"unicode/utf8"
)

const minFuzzySearchRunes = 3

var errSearchTermTooShort = errors.New("search term too short")

func validateFuzzySearchTerm(field, value string) error {
	if value == "" {
		return nil
	}
	if utf8.RuneCountInString(value) < minFuzzySearchRunes {
		return fmt.Errorf("%s must be at least %d characters: %w", field, minFuzzySearchRunes, errSearchTermTooShort)
	}
	return nil
}
