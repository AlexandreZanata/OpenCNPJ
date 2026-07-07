package perfvalidation

// Phase9SecurityDoc is the SaaS security hardening section in SECURITY.md.
const Phase9SecurityDoc = "docs/SECURITY.md"

// Phase9GateScript runs local security hardening checks.
const Phase9GateScript = "scripts/security_hardening_gate.sh"

// Phase9CodeMarkers must exist in the hardened codebase.
var Phase9CodeMarkers = []string{
	"internal/saas/hash_compare.go",
	"internal/saas/mask.go",
	"internal/middleware/metrics_auth.go",
	"internal/handlers/admin/csrf.go",
	"internal/adminauth/audit/audit.go",
}
