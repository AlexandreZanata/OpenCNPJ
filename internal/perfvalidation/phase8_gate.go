package perfvalidation

// Phase8OpenAPIFile is the public API contract (plan SaaS Phase 8).
const Phase8OpenAPIFile = "docs/api/OPENAPI.yaml"

// Phase8DocFiles are customer-facing API documentation deliverables.
var Phase8DocFiles = []string{
	"docs/api/OPENAPI.yaml",
	"docs/api/QUICKSTART.md",
	"docs/api/ERRORS.md",
	"docs/api/CHANGELOG.md",
}

// Phase8GateScript validates OpenAPI and scans for committed secrets.
const Phase8GateScript = "scripts/api_docs_gate.sh"
