#!/usr/bin/env bash
# Compression/layout experiment matrix on a deterministic sample of a kept partition.
#
#   opencnpj build --input RAW --output X.ocnpj --tmp WORK --keep-partition   # once
#   scripts/experiments.sh BIN WORK OUT [SAMPLE_EVERY]
#
# Every variant is rebuilt from the same phase-A output (`--from-partition`), restricted
# to buckets with id % SAMPLE_EVERY == 0, verified, then benchmarked on lookups.
# Results: OUT/experiments.csv (one row per variant).
set -euo pipefail
BIN=${1:?usage: experiments.sh BIN WORK OUT [SAMPLE_EVERY]}
WORK=${2:?}
OUT=${3:?}
EVERY=${4:-10}
mkdir -p "$OUT"
csv="$OUT/experiments.csv"
echo "variant,codec,block_kib,dict_kib,plain,file_bytes,data_bytes,postings_bytes,terms_bytes,documents,encode_s,lookup_p50_ms,lookup_p95_ms,lookup_p99_ms,lookup_qps_1t" > "$csv"

run() { # name codec block_kib dict_kib [--plain-columns]
  local name=$1 codec=$2 block=$3 dict=$4 plain=${5:-}
  local snap="$OUT/$name.ocnpj"
  "$BIN" build --input /nonexistent --competence 2026-10 --output "$snap" --tmp "$WORK" --from-partition \
    --sample-every "$EVERY" --codec "$codec" --block-kib "$block" --dict-kib "$dict" $plain --report "$OUT/$name.build.json" 2> "$OUT/$name.build.log"
  "$BIN" verify "$snap" 2> "$OUT/$name.verify.log"
  "$BIN" bench --snapshot "$snap" --queries 2000 --threads 1 --only lookup_cnpj --out "$OUT/$name.bench" 2> "$OUT/$name.bench.log"
  python3 -I - "$OUT/$name.build.json" "$OUT/$name.bench/bench.json" "$name" "$codec" "$block" "$dict" "${plain:+1}" >> "$csv" <<'PY'
import json, sys
b = json.load(open(sys.argv[1])); r = json.load(open(sys.argv[2]))
row = next(x for x in r["results"] if x["category"] == "lookup_cnpj" and x["cache"] == "warm")
s = b["section_bytes"]
print(",".join(map(str, [sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6], sys.argv[7] or 0, b["file_bytes"], s["data"],
    s["postings"], s["terms"], b["counts"]["documents"], round(b["timings_s"]["encode_blocks"], 2),
    round(row["p50_ms"], 4), round(row["p95_ms"], 4), round(row["p99_ms"], 4), round(row["qps"])])))
PY
  rm -f "$snap" "$snap.sha256"
  tail -1 "$csv"
}

# Block size sweep (zstd-9 + trained dictionary).
for kib in 4 16 64 256 1024; do run "z9-d112-b$kib" zstd9 "$kib" 112; done
# Codec sweep at 64 KiB.
for c in none lz4 zstd1 zstd3 zstd6 zstd12 zstd15 zstd19; do run "$c-d112-b64" "$c" 64 112; done
# Dictionary effect.
for kib in 16 64 256; do run "z9-d0-b$kib" zstd9 "$kib" 0; done
run z3-d0-b16 zstd3 16 0
run z9-d256-b64 zstd9 64 256
# Large-block archival candidates.
run z19-d0-b256 zstd19 256 0
run z19-d112-b256 zstd19 256 112
run z19-d0-b1024 zstd19 1024 0
# Typed column codecs vs plain text.
run plain-z9-d112-b64 zstd9 64 112 --plain-columns
run plain-z19-d0-b256 zstd19 256 0 --plain-columns
echo "done: $csv"
