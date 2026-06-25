package perfvalidation

// Phase3RequiredMetrics lists Prometheus series for Ristretto L1 (plan 02 Phase 3).
var Phase3RequiredMetrics = []string{
	"busca_cnpj_l1_cache_hits_total",
	"busca_cnpj_l1_cache_misses_total",
}

// Phase3ConfigKeys are cache.l1_* settings required in config defaults.
var Phase3ConfigKeys = []string{
	"l1_enabled",
	"l1_max_cost_mb",
	"l1_num_counters",
	"l1_buffer_items",
}
