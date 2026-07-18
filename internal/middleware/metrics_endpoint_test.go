package middleware

import "testing"

func TestNormalizeMetricsEndpoint(t *testing.T) {
	cases := []struct {
		route, raw, want string
	}{
		{"/api/v1/cnpj/:cnpj", "/api/v1/cnpj/00000000000191", "/api/v1/cnpj/:cnpj"},
		{"", "/api/v1/cnpj/37511144000112", "/api/v1/cnpj/:cnpj"},
		{"/readyz", "/readyz", "/readyz"},
		{"", "/admin/api/v1/me", "/admin/api/v1/me"},
	}
	for _, tc := range cases {
		if got := normalizeMetricsEndpoint(tc.route, tc.raw); got != tc.want {
			t.Fatalf("normalize(%q,%q)=%q want %q", tc.route, tc.raw, got, tc.want)
		}
	}
}
