package cnpj

import (
	cnpjdb "busca-cnpj-2026/internal/db/cnpj"
	"busca-cnpj-2026/internal/database"
	"busca-cnpj-2026/internal/services"
)

// NewDefaultLookupService builds the public lookup service from global pools.
func NewDefaultLookupService() *LookupService {
	queries := cnpjdb.New(database.CNPJPool)
	return NewLookupService(queries, services.NewCacheService())
}
