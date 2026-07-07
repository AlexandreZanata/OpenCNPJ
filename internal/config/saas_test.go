package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadSaasConfigFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "saas.yaml")
	content := []byte(`saas:
  enabled: true
  public_api_only: true
database_cnpj:
  url: "postgres://reader:secret@127.0.0.1:5432/opencnpj_cnpj"
  max_open_conns: 15
database_saas:
  url: "postgres://saas:secret@127.0.0.1:5432/opencnpj_saas"
  max_open_conns: 10
redis:
  url: "redis://127.0.0.1:6381/0"
rate_limit:
  max_requests: 3000
  window_seconds: 30
server:
  port: 8081
`)
	if err := os.WriteFile(path, content, 0o600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("CONFIG_FILE", path)
	t.Setenv("CNPJ_DATABASE_URL", "")
	t.Setenv("SAAS_DATABASE_URL", "")
	t.Setenv("REDIS_URL", "")

	if err := Load(); err != nil {
		t.Fatalf("Load: %v", err)
	}
	if !AppConfig.SaaS.Enabled || !AppConfig.SaaS.PublicAPIOnly {
		t.Fatalf("saas flags not loaded: %+v", AppConfig.SaaS)
	}
	if AppConfig.Server.Port != 8081 {
		t.Fatalf("port = %d, want 8081", AppConfig.Server.Port)
	}
	if AppConfig.Server.RateLimitMax != 3000 {
		t.Fatalf("rate limit max = %d, want 3000", AppConfig.Server.RateLimitMax)
	}
	if AppConfig.DatabaseCNPJ.URL == "" || AppConfig.DatabaseSaaS.URL == "" {
		t.Fatal("dual database urls not loaded")
	}
	if AppConfig.Redis.Host != "127.0.0.1" || AppConfig.Redis.Port != 6381 || AppConfig.Redis.DB != 0 {
		t.Fatalf("redis = %+v", AppConfig.Redis)
	}
}

func TestGetSaaSDatabaseURLFromEnv(t *testing.T) {
	AppConfig = &Config{SaaS: SaasConfig{Enabled: false}}
	t.Setenv("SAAS_DATABASE_URL", "postgres://saas:pw@127.0.0.1:5432/opencnpj_saas")
	got := GetSaaSDatabaseURL()
	want := "postgres://saas:pw@127.0.0.1:5432/opencnpj_saas"
	if got != want {
		t.Fatalf("url = %q, want %q", got, want)
	}
}

func TestParseRedisURL(t *testing.T) {
	host, port, password, db, err := parseRedisURL("redis://:s3cret@10.0.0.5:6381/2")
	if err != nil {
		t.Fatal(err)
	}
	if host != "10.0.0.5" || port != 6381 || password != "s3cret" || db != 2 {
		t.Fatalf("got host=%s port=%d pass=%q db=%d", host, port, password, db)
	}
}
