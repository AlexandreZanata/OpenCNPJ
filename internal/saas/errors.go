package saas

import "errors"

var (
	// ErrMissingKey is returned when X-API-Key header is absent.
	ErrMissingKey = errors.New("missing api key")
	// ErrInvalidKey is returned when the key format or hash does not match.
	ErrInvalidKey = errors.New("invalid api key")
	// ErrExpiredKey is returned when the key past expires_at.
	ErrExpiredKey = errors.New("expired api key")
	// ErrSuspendedClient is returned when the client status is not active.
	ErrSuspendedClient = errors.New("client suspended")
	// ErrRateLimited is returned when per-client rate limit is exceeded.
	ErrRateLimited = errors.New("rate limit exceeded")
	// ErrQuotaExceeded is returned when monthly quota is exceeded.
	ErrQuotaExceeded = errors.New("monthly quota exceeded")
)
