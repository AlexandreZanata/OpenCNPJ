package services

import (
	"testing"

	"busca-cnpj-2026/internal/models"
)

func TestMarshalCacheValueRoundTrip(t *testing.T) {
	original := models.SearchResponse{
		Total:  42,
		Limit:  10,
		Offset: 0,
	}

	data, err := marshalCacheValue(original)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	if data[0] != cacheFormatMsgpack {
		t.Fatalf("expected msgpack prefix byte, got %d", data[0])
	}

	var decoded models.SearchResponse
	if err := unmarshalCacheValue(data, &decoded); err != nil {
		t.Fatalf("unmarshal msgpack: %v", err)
	}
	if decoded.Total != original.Total {
		t.Fatalf("total = %d, want %d", decoded.Total, original.Total)
	}
}

func TestUnmarshalCacheValueLegacyJSON(t *testing.T) {
	legacy := []byte(`{"total":7,"limit":5,"offset":0}`)
	var decoded models.SearchResponse
	if err := unmarshalCacheValue(legacy, &decoded); err != nil {
		t.Fatalf("unmarshal json: %v", err)
	}
	if decoded.Total != 7 {
		t.Fatalf("total = %d, want 7", decoded.Total)
	}
}
