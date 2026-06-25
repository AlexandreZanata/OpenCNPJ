package partition

// BrazilianUFs are estado codes for estabelecimentos LIST partitioning (plan 02 Phase 6).
var BrazilianUFs = []string{
	"AC", "AL", "AP", "AM", "BA", "CE", "DF", "ES", "GO", "MA",
	"MT", "MS", "MG", "PA", "PB", "PR", "PE", "PI", "RJ", "RN",
	"RS", "RO", "RR", "SC", "SP", "SE", "TO", "EX",
}

// EstabelecimentosPartitionKey is the LIST partition column on estabelecimentos.
const EstabelecimentosPartitionKey = "uf"

// MinUFPartitions is estado codes + EX + DEFAULT partition (28 + 1).
const MinUFPartitions = 29
