package perfvalidation

// Phase4MaterializedViews lists analytics and lookup MVs (plan 02 Phase 4).
var Phase4MaterializedViews = []string{
	"mv_stats_estabelecimentos_by_uf",
	"mv_stats_estabelecimentos_by_cnae",
	"mv_stats_estabelecimentos_by_cnae_uf",
	"mv_lookup_cnaes",
	"mv_lookup_municipios",
}

// Phase4MigrationFile is the migration adding materialized views.
const Phase4MigrationFile = "migrations/000013_materialized_views.up.sql"

// Phase4RefreshFunction is the SQL entrypoint for scheduled refresh.
const Phase4RefreshFunction = "refresh_estabelecimento_stats"
