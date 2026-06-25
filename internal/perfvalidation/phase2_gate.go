package perfvalidation

// Phase2DeployFiles are VPS example templates tracked in git (plan 02 Phase 2).
var Phase2DeployFiles = []string{
	"deploy/vps/postgresql-opencnpj.conf.example",
	"deploy/vps/postgresql-autovacuum-opencnpj.conf.example",
	"deploy/vps/analyze-search-tables.sql.example",
	"docs/ops/VPS-POSTGRESQL.md",
}

// Phase2GUCExpectations maps production GUC names to example template values.
// STRICT_VPS gate compares normalized SHOW output from live Postgres on the host.
var Phase2GUCExpectations = map[string]string{
	"shared_buffers":       "4GB",
	"effective_cache_size": "12GB",
	"work_mem":             "64MB",
	"maintenance_work_mem": "2GB",
	"autovacuum":           "on",
	"wal_level":            "replica",
	"full_page_writes":     "on",
}

// Phase2ForbiddenGUCAssignments must not appear in example postgresql templates.
var Phase2ForbiddenGUCAssignments = []string{
	"autovacuum=off",
	"full_page_writes=off",
	"wal_level=minimal",
	"fsync=off",
	"synchronous_commit=off",
}
