package autherr

import "errors"

// Sentinel errors for admin auth flows.
var (
	ErrInvalidCredentials = errors.New("invalid credentials")
	ErrAccountLocked      = errors.New("account temporarily locked")
	ErrMFARequired        = errors.New("mfa required")
	ErrInvalidMFA         = errors.New("invalid mfa code")
	ErrInvalidChallenge   = errors.New("invalid or expired challenge")
	ErrInvalidToken       = errors.New("invalid token")
	ErrMFANotVerified     = errors.New("mfa not verified")
	ErrAdminDisabled      = errors.New("admin auth disabled")
)
