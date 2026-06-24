package importer

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os"

	"busca-cnpj-2026/internal/parser"
)

func scanCodes(ctx context.Context, path string, load func(context.Context, []string)) error {
	f, err := os.Open(path)
	if err != nil {
		return fmt.Errorf("open %s: %w", path, err)
	}
	defer f.Close()

	reader := parser.NewCSVReader(bufio.NewReader(f))
	var codes []string
	for {
		if err := ctx.Err(); err != nil {
			return err
		}
		line, err := reader.Read()
		if err == io.EOF {
			break
		}
		if err != nil || len(line) == 0 {
			continue
		}
		codes = append(codes, line[0])
	}
	load(ctx, codes)
	return nil
}
