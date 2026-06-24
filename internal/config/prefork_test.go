package config

import "testing"

func TestPreforkDefaultFalse(t *testing.T) {
	if err := Load(); err != nil {
		t.Fatalf("load: %v", err)
	}
	if AppConfig.Server.Prefork {
		t.Fatal("prefork must default false — DB/Redis init runs once before Fiber fork")
	}
}
