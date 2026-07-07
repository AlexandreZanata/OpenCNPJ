package cnpj

import (
	"context"
	"errors"
	"fmt"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgtype"
	"golang.org/x/sync/errgroup"

	cnpjdb "busca-cnpj-2026/internal/db/cnpj"
	"busca-cnpj-2026/internal/services"
)

// LookupService loads public CNPJ payloads via sqlc + pgx with cache.
type LookupService struct {
	queries cnpjdb.Querier
	cache   *services.CacheService
}

// NewLookupService returns a CNPJ lookup service.
func NewLookupService(queries cnpjdb.Querier, cache *services.CacheService) *LookupService {
	return &LookupService{queries: queries, cache: cache}
}

// Lookup returns the public DTO for a validated 14-digit CNPJ.
func (s *LookupService) Lookup(ctx context.Context, raw string) (*PublicResponse, error) {
	cnpj := Normalize(raw)
	if err := Validate(cnpj); err != nil {
		return nil, err
	}
	cacheKey := "public:cnpj:v1:" + cnpj
	return services.GetOrSetJSON(ctx, s.cache, cacheKey, func() (*PublicResponse, error) {
		return s.fetchParallel(ctx, cnpj)
	})
}

func (s *LookupService) fetchParallel(ctx context.Context, cnpj string) (*PublicResponse, error) {
	basico := BasicoFromCompleto(cnpj)
	var (
		est     cnpjdb.GetEstabelecimentoByCNPJRow
		emp     cnpjdb.GetEmpresaByBasicoRow
		socios  []cnpjdb.ListSociosByBasicoRow
		simples *cnpjdb.GetSimplesByBasicoRow
	)

	g, gctx := errgroup.WithContext(ctx)
	g.Go(func() error {
		row, err := s.queries.GetEstabelecimentoByCNPJ(gctx, textArg(cnpj))
		if errors.Is(err, pgx.ErrNoRows) {
			return ErrNotFound
		}
		if err != nil {
			return fmt.Errorf("estabelecimento: %w", err)
		}
		est = row
		return nil
	})
	g.Go(func() error {
		row, err := s.queries.GetEmpresaByBasico(gctx, basico)
		if errors.Is(err, pgx.ErrNoRows) {
			return nil
		}
		if err != nil {
			return fmt.Errorf("empresa: %w", err)
		}
		emp = row
		return nil
	})
	g.Go(func() error {
		rows, err := s.queries.ListSociosByBasico(gctx, basico)
		if err != nil {
			return fmt.Errorf("socios: %w", err)
		}
		socios = rows
		return nil
	})
	g.Go(func() error {
		row, err := s.queries.GetSimplesByBasico(gctx, basico)
		if errors.Is(err, pgx.ErrNoRows) {
			return nil
		}
		if err != nil {
			return fmt.Errorf("simples: %w", err)
		}
		simples = &row
		return nil
	})
	if err := g.Wait(); err != nil {
		return nil, err
	}
	resp := buildPublicResponse(est, emp, socios, simples)
	return &resp, nil
}

func textArg(s string) pgtype.Text {
	return pgtype.Text{String: s, Valid: true}
}
