package perfvalidation

// Phase0RequiredMetrics lists Prometheus series required before plan 02 optimizations.
var Phase0RequiredMetrics = []string{
	"busca_cnpj_cache_hits_total",
	"busca_cnpj_cache_misses_total",
}
