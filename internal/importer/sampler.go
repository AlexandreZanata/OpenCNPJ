package importer

import "hash/crc32"

// InSample returns true when cnpjBasico falls in the deterministic sample bucket.
func InSample(cnpjBasico string, percent int) bool {
	if percent >= 100 {
		return true
	}
	if percent <= 0 {
		return false
	}
	return crc32.ChecksumIEEE([]byte(cnpjBasico))%100 < uint32(percent)
}
