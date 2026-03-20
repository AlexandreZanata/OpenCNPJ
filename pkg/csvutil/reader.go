package csvutil

import (
	"bufio"
	"encoding/csv"
	"io"

	"golang.org/x/text/encoding/charmap"
)

const DefaultBufferSize = 4 * 1024 * 1024

func NewReader(r io.Reader) *csv.Reader {
	csvReader := csv.NewReader(charmap.ISO8859_1.NewDecoder().Reader(bufio.NewReaderSize(r, DefaultBufferSize)))
	csvReader.Comma = ';'
	csvReader.LazyQuotes = true
	csvReader.FieldsPerRecord = -1
	return csvReader
}
