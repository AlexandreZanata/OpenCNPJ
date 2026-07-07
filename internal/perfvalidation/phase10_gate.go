package perfvalidation

// Phase10DeployRunbook is the operator production deploy guide.
const Phase10DeployRunbook = "docs/ops/DEPLOY-RUNBOOK.md"

// Phase10GateScript validates deploy artifacts and optional docker smoke.
const Phase10GateScript = "scripts/saas_deploy_gate.sh"

// Phase10SmokeScript is the post-deploy health check.
const Phase10SmokeScript = "scripts/saas_smoke.sh"

// Phase10DeployArtifacts must exist for the runbook gate.
var Phase10DeployArtifacts = []string{
	"docs/ops/DEPLOY-RUNBOOK.md",
	"deploy/saas/systemd-opencnpj-api.example",
	"deploy/saas/api.env.example",
	"deploy/saas/redis-opencnpj.conf.example",
	"deploy/saas/rollback.example.sh",
	"scripts/build_opencnpj_api.sh",
	"scripts/saas_smoke.sh",
}
