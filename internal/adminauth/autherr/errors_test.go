package autherr_test

import (
	"errors"
	"testing"

	"busca-cnpj-2026/internal/adminauth/autherr"
)

func TestSentinelErrorsDistinct(t *testing.T) {
	errs := []error{
		autherr.ErrInvalidCredentials,
		autherr.ErrAccountLocked,
		autherr.ErrMFARequired,
		autherr.ErrInvalidMFA,
		autherr.ErrInvalidChallenge,
		autherr.ErrInvalidToken,
		autherr.ErrMFANotVerified,
		autherr.ErrAdminDisabled,
	}
	for i, a := range errs {
		for j, b := range errs {
			if i != j && errors.Is(a, b) {
				t.Fatalf("errors %d and %d should differ", i, j)
			}
		}
	}
}
