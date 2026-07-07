package cnpj

import "context"

// Lookuper loads public CNPJ responses.
type Lookuper interface {
	Lookup(ctx context.Context, raw string) (*PublicResponse, error)
}
