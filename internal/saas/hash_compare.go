package saas

import "crypto/subtle"

// SecureCompareKeyHash compares API key digests in constant time.
func SecureCompareKeyHash(stored, computed []byte) bool {
	if len(stored) != sha256Size || len(computed) != sha256Size {
		dummy := make([]byte, sha256Size)
		if len(computed) == sha256Size {
			_ = subtle.ConstantTimeCompare(dummy, computed)
		}
		return false
	}
	return subtle.ConstantTimeCompare(stored, computed) == 1
}

const sha256Size = 32
