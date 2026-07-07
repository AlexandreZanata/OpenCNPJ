package adminauth

import (
	"encoding/base64"
	"fmt"
)

func decodeBase64Key(raw string) ([]byte, error) {
	key, err := base64.StdEncoding.DecodeString(raw)
	if err != nil {
		key, err = base64.RawStdEncoding.DecodeString(raw)
		if err != nil {
			return nil, fmt.Errorf("decode MFA_SECRET_ENCRYPTION_KEY: %w", err)
		}
	}
	if len(key) != 32 {
		return nil, fmt.Errorf("MFA_SECRET_ENCRYPTION_KEY must be 32 bytes (got %d)", len(key))
	}
	return key, nil
}
