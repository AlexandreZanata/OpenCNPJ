package totp

import (
	"fmt"
	"time"

	"github.com/pquerna/otp"
	"github.com/pquerna/otp/totp"
)

// Service generates and validates TOTP codes.
type Service struct {
	issuer string
}

// NewService returns a TOTP helper for the given issuer name.
func NewService(issuer string) *Service {
	return &Service{issuer: issuer}
}

// Generate creates a new base32 secret and otpauth URL for QR provisioning.
func (s *Service) Generate(account string) (secret, otpauthURL string, err error) {
	key, err := totp.Generate(totp.GenerateOpts{
		Issuer:      s.issuer,
		AccountName: account,
		Period:      30,
		Digits:      otp.DigitsSix,
		Algorithm:   otp.AlgorithmSHA1,
	})
	if err != nil {
		return "", "", fmt.Errorf("generate totp: %w", err)
	}
	return key.Secret(), key.URL(), nil
}

// Validate checks a 6-digit TOTP code against the base32 secret.
func (s *Service) Validate(secret, code string) bool {
	return totp.Validate(code, secret)
}

// CurrentCode returns the valid code now (tests only).
func (s *Service) CurrentCode(secret string) (string, error) {
	return totp.GenerateCode(secret, time.Now())
}
