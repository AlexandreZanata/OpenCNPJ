package services

import (
	"encoding/json"
	"fmt"

	"github.com/vmihailenco/msgpack/v5"
)

const cacheFormatMsgpack byte = 0x01

func marshalCacheValue(value interface{}) ([]byte, error) {
	payload, err := msgpack.Marshal(value)
	if err != nil {
		return nil, fmt.Errorf("msgpack marshal: %w", err)
	}
	out := make([]byte, 1+len(payload))
	out[0] = cacheFormatMsgpack
	copy(out[1:], payload)
	return out, nil
}

func unmarshalCacheValue(data []byte, dest interface{}) error {
	if len(data) > 0 && data[0] == cacheFormatMsgpack {
		if err := msgpack.Unmarshal(data[1:], dest); err != nil {
			return fmt.Errorf("msgpack unmarshal: %w", err)
		}
		return nil
	}
	if err := json.Unmarshal(data, dest); err != nil {
		return fmt.Errorf("json unmarshal: %w", err)
	}
	return nil
}
