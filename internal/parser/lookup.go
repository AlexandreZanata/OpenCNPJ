package parser

import (
	"context"
	"sync"
)

type LookupStore struct {
	mu         sync.RWMutex
	naturezas  map[string]struct{}
	municipios map[string]struct{}
	paises     map[string]struct{}
	cnaes      map[string]struct{}
}

func NewLookupStore() *LookupStore {
	return &LookupStore{
		naturezas:  make(map[string]struct{}),
		municipios: make(map[string]struct{}),
		paises:     make(map[string]struct{}),
		cnaes:      make(map[string]struct{}),
	}
}

func (s *LookupStore) LoadNaturezas(_ context.Context, values []string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, v := range values {
		s.naturezas[v] = struct{}{}
	}
}

func (s *LookupStore) LoadMunicipios(_ context.Context, values []string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, v := range values {
		s.municipios[v] = struct{}{}
	}
}

func (s *LookupStore) LoadPaises(_ context.Context, values []string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, v := range values {
		s.paises[v] = struct{}{}
	}
}

func (s *LookupStore) LoadCNAEs(_ context.Context, values []string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, v := range values {
		s.cnaes[v] = struct{}{}
	}
}

func (s *LookupStore) ExistsNatureza(v string) bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	_, ok := s.naturezas[v]
	return ok
}

func (s *LookupStore) ExistsMunicipio(v string) bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	_, ok := s.municipios[v]
	return ok
}

func (s *LookupStore) ExistsPais(v string) bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	_, ok := s.paises[v]
	return ok
}

func (s *LookupStore) ExistsCNAE(v string) bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	_, ok := s.cnaes[v]
	return ok
}
