#!/usr/bin/env python3
"""API consistency checks and query throughput benchmark."""
from __future__ import annotations

import concurrent.futures
import json
import statistics
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

API = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
DURATION = int(sys.argv[2]) if len(sys.argv) > 2 else 20
CONCURRENCY = int(sys.argv[3]) if len(sys.argv) > 3 else 10
REPORT = sys.argv[4] if len(sys.argv) > 4 else "/tmp/api_audit_report.txt"


def db_sample() -> dict[str, str]:
    sql = """
    SELECT e.cnpj_completo, e.cnpj_basico, e.uf, e.cnae_fiscal_principal, emp.razao_social
    FROM estabelecimentos e
    JOIN empresas emp ON e.cnpj_basico = emp.cnpj_basico
    WHERE length(e.cnpj_completo) = 14
    LIMIT 1;
    """
    out = subprocess.check_output(
        ["docker", "exec", "receita-postgres", "psql", "-U", "receita_user", "-d", "receita_db", "-t", "-A", "-F|", "-c", sql],
        text=True,
    ).strip()
    parts = out.split("|")
    return {
        "cnpj": parts[0].strip(),
        "cnpj_basico": parts[1].strip(),
        "uf": parts[2].strip(),
        "cnae": parts[3].strip(),
        "razao_social": parts[4].strip() if len(parts) > 4 else "",
    }


def db_counts() -> dict[str, int]:
    sql = """
    SELECT 'empresas', COUNT(*) FROM empresas
    UNION ALL SELECT 'estabelecimentos', COUNT(*) FROM estabelecimentos
    UNION ALL SELECT 'socios', COUNT(*) FROM socios
    UNION ALL SELECT 'simples', COUNT(*) FROM simples;
    """
    out = subprocess.check_output(
        ["docker", "exec", "receita-postgres", "psql", "-U", "receita_user", "-d", "receita_db", "-t", "-A", "-c", sql],
        text=True,
    )
    counts: dict[str, int] = {}
    for line in out.splitlines():
        if "|" in line:
            table, value = line.split("|", 1)
            counts[table.strip()] = int(value.strip())
    return counts


def get(path: str) -> tuple[int, bytes, float]:
    url = API + path
    start = time.perf_counter()
    with urllib.request.urlopen(url, timeout=120) as resp:
        body = resp.read()
        code = resp.status
    return code, body, (time.perf_counter() - start) * 1000


def post_json(path: str, payload: dict) -> tuple[int, bytes, float]:
    url = API + path
    data = json.dumps(payload).encode()
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"}, method="POST")
    start = time.perf_counter()
    with urllib.request.urlopen(req, timeout=120) as resp:
        body = resp.read()
        code = resp.status
    return code, body, (time.perf_counter() - start) * 1000


def field(value) -> str:
    if isinstance(value, dict) and "String" in value:
        return str(value.get("String", "")).strip()
    return str(value or "").strip()


def run_consistency(sample: dict, counts: dict) -> tuple[list[dict], list[str]]:
    checks: list[dict] = []
    errors: list[str] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        checks.append({"check": name, "ok": ok, "detail": detail})

    try:
        code, _, ms = get("/")
        check("GET /", code == 200, f"status={code} {ms:.0f}ms")

        code, _, ms = get("/readyz")
        check("GET /readyz", code == 200, f"status={code} {ms:.0f}ms")

        cnpj = sample["cnpj"]
        cnpj_basico = sample["cnpj_basico"]

        code, body, ms = get(f"/api/v1/estabelecimentos/{cnpj}")
        est = json.loads(body)
        check(
            "GET /estabelecimentos/:cnpj",
            code == 200 and field(est.get("cnpj_completo")) == cnpj,
            f"returned={field(est.get('cnpj_completo'))} {ms:.0f}ms",
        )

        code, body, ms = get(f"/api/v1/estabelecimentos/search?cnpj={cnpj}&limit=1")
        search = json.loads(body)
        data = search.get("data") or []
        check(
            "GET /estabelecimentos/search?cnpj",
            code == 200 and len(data) >= 1 and field(data[0].get("cnpj_completo")) == cnpj,
            f"total={search.get('total')} {ms:.0f}ms",
        )

        code, body, ms = get(f"/api/v1/empresas/search?cnpj_basico={cnpj_basico}&limit=1")
        emp = json.loads(body)
        edata = emp.get("data") or []
        check(
            "GET /empresas/search?cnpj_basico",
            code == 200 and len(edata) == 1 and field(edata[0].get("cnpj_basico")) == cnpj_basico,
            f"total={emp.get('total')} {ms:.0f}ms",
        )

        api_razao = field(edata[0].get("razao_social")) if edata else ""
        est_razao = field(est.get("razao_social"))
        check(
            "empresa-estabelecimento join",
            api_razao == est_razao or est_razao == sample["razao_social"],
            f"match={api_razao[:60]}",
        )

        code, body, ms = get("/api/v1/empresas/search?razao_social=LTDA&limit=10")
        fuzzy = json.loads(body)
        check(
            "GET /empresas/search fuzzy",
            code == 200 and fuzzy.get("total", 0) > 0,
            f"total={fuzzy.get('total')} rows={len(fuzzy.get('data') or [])} {ms:.0f}ms",
        )

        q = urllib.parse.urlencode({"uf": sample["uf"], "cnae": sample["cnae"], "limit": "5"})
        code, body, ms = get(f"/api/v1/estabelecimentos/search?{q}")
        filt = json.loads(body)
        fdata = filt.get("data") or []
        check(
            "GET /estabelecimentos/search filtered",
            code == 200 and len(fdata) > 0,
            f"total={filt.get('total')} uf={sample['uf']} cnae={sample['cnae']} {ms:.0f}ms",
        )

        code, body, ms = get("/api/v1/estabelecimentos/search?nome_fantasia=MERCADO&limit=5")
        nf = json.loads(body)
        check(
            "GET /estabelecimentos/search nome_fantasia",
            code == 200,
            f"total={nf.get('total')} {ms:.0f}ms",
        )

        code, body, ms = get("/api/v1/stats/cnae?limit=5")
        stats_cnae = json.loads(body)
        check(
            "GET /stats/cnae",
            code == 200 and len(stats_cnae) > 0,
            f"top={stats_cnae[0].get('cnae')} count={stats_cnae[0].get('count')} {ms:.0f}ms",
        )

        code, body, ms = get("/api/v1/stats/uf")
        stats_uf = json.loads(body)
        uf_sum = sum(int(s.get("count", 0)) for s in stats_uf)
        check(
            "GET /stats/uf",
            code == 200 and len(stats_uf) >= 27,
            f"states={len(stats_uf)} sum={uf_sum} db_estab={counts.get('estabelecimentos', 0)} {ms:.0f}ms",
        )

        top_cnae = stats_cnae[0]["cnae"]
        code, body, ms = get(f"/api/v1/stats/cnae/{top_cnae}/uf?limit=5")
        stats_cu = json.loads(body)
        check(
            "GET /stats/cnae/:cnae/uf",
            code == 200 and len(stats_cu) > 0,
            f"cnae={top_cnae} rows={len(stats_cu)} {ms:.0f}ms",
        )

        export_body = {"filters": {"uf": "SP", "limit": 100}, "selected_columns": ["cnpj_completo", "uf"], "format": "csv"}
        code, body, ms = post_json("/api/v1/export/csv", export_body)
        lines = body.decode().strip().split("\n") if body else []
        check("POST /export/csv", code == 200 and len(lines) >= 2, f"csv_lines={len(lines)} {ms:.0f}ms")

        check(
            "DB row counts loaded",
            counts.get("empresas", 0) > 60_000_000,
            str(counts),
        )

    except Exception as exc:  # noqa: BLE001
        errors.append(str(exc))

    return checks, errors


def one_request(path: str) -> tuple[float, bool]:
    start = time.perf_counter()
    try:
        with urllib.request.urlopen(API + path, timeout=120) as resp:
            resp.read()
            ok = resp.status == 200
    except (urllib.error.URLError, TimeoutError):
        ok = False
    return (time.perf_counter() - start) * 1000, ok


def benchmark_route(label: str, path: str) -> dict:
    for _ in range(3):
        one_request(path)

    end = time.time() + DURATION
    latencies: list[float] = []
    ok_count = 0
    total = 0

    with concurrent.futures.ThreadPoolExecutor(max_workers=CONCURRENCY) as pool:
        pending: set[concurrent.futures.Future] = set()
        while time.time() < end or pending:
            while len(pending) < CONCURRENCY * 2 and time.time() < end:
                pending.add(pool.submit(one_request, path))
            if not pending:
                break
            done, pending = concurrent.futures.wait(pending, timeout=0.05, return_when=concurrent.futures.FIRST_COMPLETED)
            for fut in done:
                ms, ok = fut.result()
                latencies.append(ms)
                total += 1
                if ok:
                    ok_count += 1

    latencies.sort()
    p50 = latencies[len(latencies) // 2] if latencies else 0.0
    p95 = latencies[int(len(latencies) * 0.95)] if latencies else 0.0
    return {
        "route": label,
        "path": path,
        "duration_s": DURATION,
        "concurrency": CONCURRENCY,
        "requests": total,
        "ok": ok_count,
        "rps": round(total / DURATION, 1) if DURATION else 0.0,
        "p50_ms": round(p50, 1),
        "p95_ms": round(p95, 1),
    }


def main() -> int:
    sample = db_sample()
    counts = db_counts()
    checks, errors = run_consistency(sample, counts)

    bench_routes = [
        ("cnpj_lookup", f"/api/v1/estabelecimentos/{sample['cnpj']}"),
        ("cnpj_search", f"/api/v1/estabelecimentos/search?cnpj={sample['cnpj']}&limit=1"),
        ("empresa_fuzzy", "/api/v1/empresas/search?razao_social=LTDA&limit=20"),
        ("estab_filter", "/api/v1/estabelecimentos/search?uf=SP&cnae=6201500&limit=50"),
        ("stats_uf", "/api/v1/stats/uf"),
    ]
    performance = [benchmark_route(label, path) for label, path in bench_routes]

    passed = sum(1 for c in checks if c["ok"])
    failed = sum(1 for c in checks if not c["ok"])

    lines = [
        "=" * 62,
        " API AUDIT REPORT",
        "=" * 62,
        f"API:         {API}",
        f"Sample CNPJ: {sample['cnpj']} (basico={sample['cnpj_basico']}, uf={sample['uf']})",
        "",
        f"CONSISTENCY: {passed} passed, {failed} failed",
    ]
    for c in checks:
        status = "PASS" if c["ok"] else "FAIL"
        lines.append(f"  [{status}] {c['check']}: {c['detail']}")
    if errors:
        lines.append("ERRORS:")
        for err in errors:
            lines.append(f"  - {err}")

    lines.extend([
        "",
        f"PERFORMANCE ({DURATION}s @ concurrency={CONCURRENCY})",
        f"{'Route':<16} {'RPS':>8} {'p50 ms':>10} {'p95 ms':>10} {'OK':>12}",
    ])
    for p in performance:
        lines.append(
            f"{p['route']:<16} {p['rps']:>8.1f} {p['p50_ms']:>10.1f} {p['p95_ms']:>10.1f} {p['ok']:>5}/{p['requests']}"
        )

    report = "\n".join(lines) + "\n"
    print(report)
    with open(REPORT, "w", encoding="utf-8") as fh:
        fh.write(report)

    return 1 if failed or errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
