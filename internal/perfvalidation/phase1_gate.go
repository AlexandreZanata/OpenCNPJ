package perfvalidation

// Phase1DeployFiles are committed VPS tuning artifacts (plan 02 Phase 1).
var Phase1DeployFiles = []string{
	"deploy/vps/sysctl-opencnpj.conf",
	"deploy/vps/limits-postgres.conf",
	"deploy/vps/99-opencnpj-io-scheduler.rules",
	"deploy/vps/fstab-postgres.example",
	"deploy/vps/README.md",
}

// Phase1SysctlExpectations maps sysctl keys to required values on production VPS.
// Local dev may differ; STRICT_VPS gate enforces these on the host.
var Phase1SysctlExpectations = map[string]string{
	"vm.swappiness":              "1",
	"vm.dirty_ratio":             "10",
	"vm.dirty_background_ratio":  "3",
	"kernel.shmmax":              "4294967296",
	"net.core.somaxconn":         "4096",
}

// Phase1SysctlForbiddenSubstrings must not appear in production sysctl templates.
var Phase1SysctlForbiddenSubstrings = []string{
	"autovacuum",
	"full_page_writes",
	"wal_level",
}

// Phase1SwapIncreaseMaxKiB is the max allowed swap growth during light k6 load.
const Phase1SwapIncreaseMaxKiB = 102400 // 100 MiB
