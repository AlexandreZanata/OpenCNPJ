package password

import "crypto/rand"

func randRead(buf []byte) (int, error) {
	return rand.Read(buf)
}
