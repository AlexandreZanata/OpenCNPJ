package cnpj

import (
	"errors"
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

var nonDigits = regexp.MustCompile(`\D`)

// ErrInvalidCNPJ is returned when format or check digits are invalid.
var ErrInvalidCNPJ = errors.New("invalid cnpj")

// ErrNotFound is returned when the CNPJ is valid but absent from the database.
var ErrNotFound = errors.New("cnpj not found")

// Normalize strips non-digits from a CNPJ input.
func Normalize(raw string) string {
	return nonDigits.ReplaceAllString(raw, "")
}

// Validate checks length and modulo-11 check digits.
func Validate(cnpj string) error {
	cnpj = Normalize(cnpj)
	if len(cnpj) != 14 {
		return ErrInvalidCNPJ
	}
	if allSameDigit(cnpj) {
		return ErrInvalidCNPJ
	}
	d1, err := calcCNPJDigit(cnpj[:12], []int{5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2})
	if err != nil {
		return ErrInvalidCNPJ
	}
	if d1 != int(cnpj[12]-'0') {
		return ErrInvalidCNPJ
	}
	d2, err := calcCNPJDigit(cnpj[:13], []int{6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2})
	if err != nil {
		return ErrInvalidCNPJ
	}
	if d2 != int(cnpj[13]-'0') {
		return ErrInvalidCNPJ
	}
	return nil
}

func calcCNPJDigit(base string, weights []int) (int, error) {
	if len(base) != len(weights) {
		return 0, fmt.Errorf("weight mismatch")
	}
	sum := 0
	for i, w := range weights {
		n, err := strconv.Atoi(string(base[i]))
		if err != nil {
			return 0, err
		}
		sum += n * w
	}
	rem := sum % 11
	if rem < 2 {
		return 0, nil
	}
	return 11 - rem, nil
}

func allSameDigit(s string) bool {
	if s == "" {
		return true
	}
	return strings.Count(s, string(s[0])) == len(s)
}

// BasicoFromCompleto returns the 8-digit CNPJ root from a normalized 14-digit CNPJ.
func BasicoFromCompleto(cnpj string) string {
	if len(cnpj) < 8 {
		return ""
	}
	return cnpj[:8]
}
