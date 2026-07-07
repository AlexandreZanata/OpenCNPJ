package saas

import (
	"github.com/google/uuid"
)

const (
	// HeaderAPIKey is the HTTP header carrying the client API key.
	HeaderAPIKey = "X-API-Key"
	// KeyPrefix is the visible prefix for live API keys.
	KeyPrefix = "ocnpj_live_"
	// KeyHexLength is the random hex suffix length after the prefix.
	KeyHexLength = 32
	// ClientContextKey is the Fiber locals key for AuthenticatedClient.
	ClientContextKey = "saas_client"
	// ClientStatusActive is the only status allowing API access.
	ClientStatusActive = "active"
	// ClientStatusSuspended blocks API access.
	ClientStatusSuspended = "suspended"
)

// AuthenticatedClient is attached to the request after API key validation.
type AuthenticatedClient struct {
	KeyID           uuid.UUID
	ClientID        uuid.UUID
	KeyPrefix       string
	RateLimitPerMin int
	MonthlyQuota    int
	Status          string
}

// Deps bundles SaaS middleware dependencies.
type Deps struct {
	Auth      Authenticator
	RateLimit ClientRateLimiter
	Usage     UsageRecorder
}
