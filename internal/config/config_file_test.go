package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadConfigFileEnv(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "custom.yaml")
	content := []byte("server:\n  port: 9090\n")
	if err := os.WriteFile(path, content, 0o600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("CONFIG_FILE", path)
	if err := Load(); err != nil {
		t.Fatalf("Load: %v", err)
	}
	if AppConfig.Server.Port != 9090 {
		t.Fatalf("port = %d, want 9090", AppConfig.Server.Port)
	}
}
