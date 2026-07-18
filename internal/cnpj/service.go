package cnpj

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgtype"
	"golang.org/x/sync/errgroup"

	cnpjdb "busca-cnpj-2026/internal/db/cnpj"
	"busca-cnpj-2026/internal/services"
)

// publicCacheKeyPrefix is the Redis/L1 key for successful lookups (v2 envelope).
const publicCacheKeyPrefix = "public:cnpj:v2:"

// publicMissTTL is the negative-cache window for absent CNPJs (typos / scanners).
const publicMissTTL = 2 * time.Minute

// lookupCacheEntry is stored for both hits and misses.
type lookupCacheEntry struct {
	NotFound bool            `json:"nf,omitempty"`
	Payload  *PublicResponse `json:"p,omitempty"`
}

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
	key := publicCacheKeyPrefix + cnpj
	if entry, ok := s.readCache(ctx, key); ok {
		if entry.NotFound {
			return nil, ErrNotFound
		}
		return entry.Payload, nil
	}
	resp, err := s.fetchParallel(ctx, cnpj)
	if errors.Is(err, ErrNotFound) {
		s.writeMiss(ctx, key)
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, err
	}
	s.writeHit(ctx, key, resp)
	return resp, nil
}

func (s *LookupService) fetchParallel(ctx context.Context, cnpj string) (*PublicResponse, error) {
	basico := BasicoFromCompleto(cnpj)
	var (
		est     cnpjdb.GetEstabelecimentoByCNPJRow
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
	resp := buildPublicResponse(est, socios, simples)
	return &resp, nil
}

func (s *LookupService) readCache(ctx context.Context, key string) (lookupCacheEntry, bool) {
	if s.cache == nil || !s.cache.Enabled() {
		return lookupCacheEntry{}, false
	}
	var entry lookupCacheEntry
	ok, err := s.cache.GetJSON(ctx, key, &entry)
	if err != nil || !ok {
		return lookupCacheEntry{}, false
	}
	if entry.NotFound {
		return entry, true
	}
	if entry.Payload == nil {
		return lookupCacheEntry{}, false
	}
	return entry, true
}

func (s *LookupService) writeHit(ctx context.Context, key string, resp *PublicResponse) {
	if s.cache == nil || !s.cache.Enabled() {
		return
	}
	_ = s.cache.Set(ctx, key, lookupCacheEntry{Payload: resp})
}

func (s *LookupService) writeMiss(ctx context.Context, key string) {
	if s.cache == nil || !s.cache.Enabled() {
		return
	}
	_ = s.cache.SetWithTTL(ctx, key, lookupCacheEntry{NotFound: true}, publicMissTTL)
}

func textArg(s string) pgtype.Text {
	return pgtype.Text{String: s, Valid: true}
}
