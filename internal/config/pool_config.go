package config

// AsPoolConfig builds pool limits from legacy DatabaseConfig.
func (d DatabaseConfig) AsPoolConfig() PoolConfig {
	return PoolConfig{
		MaxOpenConns: d.MaxOpenConns,
		MaxIdleConns: d.MaxIdleConns,
	}
}

func (d DatabaseURLConfig) AsPoolConfig() PoolConfig {
	return PoolConfig{
		MaxOpenConns: d.MaxOpenConns,
		MaxIdleConns: d.MaxIdleConns,
	}
}
