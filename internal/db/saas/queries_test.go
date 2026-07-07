package saasdb_test

import (
	"os"
	"strings"
	"testing"
)

func TestGetAPIKeyByHashUsesPartialRevokedFilter(t *testing.T) {
	raw, err := os.ReadFile("../../../db/queries/saas/api_keys.sql")
	if err != nil {
		t.Fatal(err)
	}
	sql := string(raw)
	if !strings.Contains(sql, "WHERE k.key_hash = $1 AND k.revoked_at IS NULL") {
		t.Fatal("auth query must filter revoked_at IS NULL for partial index use")
	}
}

func TestUsageDailyUpsertTargetsPrimaryKey(t *testing.T) {
	raw, err := os.ReadFile("../../../db/queries/saas/api_usage.sql")
	if err != nil {
		t.Fatal(err)
	}
	sql := string(raw)
	if !strings.Contains(sql, "ON CONFLICT (client_id, date)") {
		t.Fatal("usage flush must upsert on api_usage_daily PK (client_id, date)")
	}
}
