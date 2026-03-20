package parser

import (
	"context"
	"encoding/csv"
	"io"
	"regexp"
	"strings"

	"github.com/shopspring/decimal"
	"golang.org/x/text/encoding/charmap"
)

var cnpjBasicoRx = regexp.MustCompile(`^\d{8}$`)

func NewCSVReader(r io.Reader) *csv.Reader {
	reader := csv.NewReader(charmap.ISO8859_1.NewDecoder().Reader(r))
	reader.Comma = ';'
	reader.LazyQuotes = true
	reader.FieldsPerRecord = -1
	return reader
}

func ValidateCNPJBasico(value string) error {
	if !cnpjBasicoRx.MatchString(value) {
		return InvalidFieldError{Field: "cnpj_basico", Value: value, Reason: "must be 8 digits"}
	}
	return nil
}

func ParseCapitalSocial(value string) (decimal.Decimal, error) {
	clean := strings.TrimSpace(value)
	clean = strings.ReplaceAll(clean, ".", "")
	clean = strings.ReplaceAll(clean, ",", ".")
	dec, err := decimal.NewFromString(clean)
	if err != nil {
		return decimal.Zero, InvalidFieldError{Field: "capital_social", Value: value, Reason: err.Error()}
	}
	return dec, nil
}

func ReadRawLines(
	ctx context.Context,
	reader *csv.Reader,
	out chan<- []string,
	errCh chan<- error,
) {
	defer close(out)
	for {
		select {
		case <-ctx.Done():
			errCh <- ctx.Err()
			return
		default:
		}

		rec, err := reader.Read()
		if err != nil {
			if err == io.EOF {
				return
			}
			errCh <- err
			return
		}
		out <- rec
	}
}
