package importer

// Options controls CSV ingestion into PostgreSQL.
type Options struct {
	DataPath             string
	SamplePercent        int
	BatchSize            int
	Workers              int
	Tune                 bool
	Benchmark            bool
	Profile              bool
	SkipRefs             bool
	SkipEmpresas         bool
	SkipEstabelecimentos bool
	SkipSocios           bool
	SkipSimples          bool
	NoClean              bool
	RefsOnly             bool
}
