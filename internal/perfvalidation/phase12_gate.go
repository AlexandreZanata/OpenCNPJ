package perfvalidation

// Phase12DataAccessDoc is the public operator guide for sqlc + pgx performance stack.
const Phase12DataAccessDoc = "docs/ops/DATA-ACCESS-PERFORMANCE.md"

// Phase12GateScript validates data-access artifacts and optional EXPLAIN integration.
const Phase12GateScript = "scripts/saas_data_access_gate.sh"

// MaxCNPJLookupFanOut is the errgroup goroutine budget for GET /api/v1/cnpj/:cnpj.
const MaxCNPJLookupFanOut = 3

// Phase12RequiredFiles must exist for the data-access gate.
var Phase12RequiredFiles = []string{
	"sqlc.yaml",
	"db/schema/cnpj.sql",
	"db/queries/cnpj/estabelecimento.sql",
	"db/queries/cnpj/empresa.sql",
	"db/queries/cnpj/socios.sql",
	"db/queries/cnpj/simples.sql",
	"db/queries/saas/api_keys.sql",
	"db/queries/saas/api_usage.sql",
	"internal/database/cnpj_pgx.go",
	"internal/database/saas_pgx.go",
	"internal/cnpj/service.go",
	"migrations/saas/000002_saas_indexes.up.sql",
}

// Phase12CNPJIndex is required in db/schema for cnpj_completo lookup EXPLAIN gate.
const Phase12CNPJIndex = "idx_estabelecimentos_cnpj_completo"

// Phase12VPSCNPJIndex is the VPS hot-path name (avoids collision with legacy partition leftovers).
const Phase12VPSCNPJIndex = "idx_estab_uf_cnpj_completo"

// Phase12VPSIndexScript creates indexes after VPS restore (UF-partitioned estabelecimentos).
const Phase12VPSIndexScript = "scripts/vps_create_indexes.sql"

// Phase12SaaSIndexes are required partial/PK indexes for auth and usage flush.
var Phase12SaaSIndexes = []string{
	"idx_api_keys_hash",
	"idx_api_clients_status_active",
	"PRIMARY KEY (client_id, date)",
}
