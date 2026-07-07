package saas_test

import (
	"os"
	"strings"
	"testing"
)

func TestSystemdUnitTemplate(t *testing.T) {
	body, err := os.ReadFile(deployPath(t, "systemd-opencnpj-api.example"))
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)
	for _, needle := range []string{
		"User=opencnpj",
		"EnvironmentFile=/etc/opencnpj/api.env",
		"ExecStart=/usr/local/bin/opencnpj-api",
		"MemoryMax=512M",
		"NoNewPrivileges=true",
	} {
		if !strings.Contains(content, needle) {
			t.Errorf("systemd template missing %q", needle)
		}
	}
}

func TestRedisTemplateBindsLocalhost(t *testing.T) {
	body, err := os.ReadFile(deployPath(t, "redis-opencnpj.conf.example"))
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)
	if !strings.Contains(content, "bind 127.0.0.1") {
		t.Fatal("redis must bind localhost only")
	}
	if !strings.Contains(content, "port 6381") || !strings.Contains(content, "maxmemory 128mb") {
		t.Fatal("redis memory/port settings missing")
	}
}

func TestRollbackScriptUsesBackupBinary(t *testing.T) {
	body, err := os.ReadFile(deployPath(t, "rollback.example.sh"))
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)
	if !strings.Contains(content, ".bak") || !strings.Contains(content, "opencnpj-api") {
		t.Fatal("rollback must restore backup binary")
	}
}
