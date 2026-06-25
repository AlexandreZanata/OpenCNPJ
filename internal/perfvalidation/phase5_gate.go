package perfvalidation

// Phase5ConfigKeys are meilisearch selective-index settings (plan 02 Phase 5).
var Phase5ConfigKeys = []string{
	"selective_active_matriz",
}

// Phase5SelectiveSQLMarkers must appear in indexer selective queries.
var Phase5SelectiveSQLMarkers = []string{
	"activeSituacaoCadastral",
	"matrizIdentificador",
	"identificador_matriz_filial",
	"situacao_cadastral",
}
