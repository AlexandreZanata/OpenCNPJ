package saas_test

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func nginxTemplatePath(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed")
	}
	return filepath.Join(filepath.Dir(file), "nginx-comerc.app.br.example")
}

func TestNginxTemplateRequiredDirectives(t *testing.T) {
	body, err := os.ReadFile(nginxTemplatePath(t))
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)

	required := []string{
		"limit_req_zone $binary_remote_addr zone=opencnpj_api",
		"upstream opencnpj_api",
		"127.0.0.1:8081",
		"server_name api.comerc.app.br",
		"server_name admin.comerc.app.br",
		"client_max_body_size 1m",
		"location = /readyz",
		"proxy_read_timeout 30s",
		"include /etc/nginx/snippets/cloudflare-real-ip.conf",
		"location /metrics",
		"deny all",
	}

	for _, needle := range required {
		if !strings.Contains(content, needle) {
			t.Errorf("nginx template missing %q", needle)
		}
	}
}

func TestCloudflareSnippetHasRealIP(t *testing.T) {
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed")
	}
	path := filepath.Join(filepath.Dir(file), "cloudflare-real-ip.conf.example")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	content := string(body)
	if !strings.Contains(content, "real_ip_header CF-Connecting-IP") {
		t.Fatal("missing CF-Connecting-IP header")
	}
	if !strings.Contains(content, "set_real_ip_from 173.245.48.0/20") {
		t.Fatal("missing Cloudflare IPv4 range")
	}
}
