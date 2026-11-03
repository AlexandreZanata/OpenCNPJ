#!/usr/bin/env python3
"""Query latency on the SQLite baseline, using the URL lists written by
`opencnpj bench --write-urls` (same sampled CNPJs and names as the engine benchmark).

Usage: python3 -I scripts/baseline_queries.py BASELINES_DIR URLS_DIR [N]"""
import re
import sqlite3
import statistics
import sys
import time
import urllib.parse

base, urls, n = sys.argv[1], sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 500
db = sqlite3.connect(f"file:{base}/cnpj.sqlite?mode=ro", uri=True)


def pct(xs, p):
    xs = sorted(xs)
    return xs[round((len(xs) - 1) * p)]


def report(name, lat):
    print(f"| sqlite {name} | {len(lat)} | {pct(lat, .5):.3f} | {pct(lat, .95):.3f} | {pct(lat, .99):.3f} |")


cnpjs = [line.rsplit("/", 1)[1].strip() for line in open(f"{urls}/lookup_cnpj.txt")][:n]
lat = []
for c in cnpjs:
    t = time.perf_counter()
    b, o, d = c[:8], c[8:12], c[12:]
    e = db.execute("SELECT * FROM estabelecimentos WHERE cnpj_basico=? AND cnpj_ordem=? AND cnpj_dv=?", (b, o, d)).fetchall()
    db.execute("SELECT * FROM empresas WHERE cnpj_basico=?", (b,)).fetchall()
    db.execute("SELECT * FROM socios WHERE cnpj_basico=?", (b,)).fetchall()
    db.execute("SELECT * FROM simples WHERE cnpj_basico=?", (b,)).fetchall()
    lat.append((time.perf_counter() - t) * 1e3)
    assert e, c
report("lookup_cnpj (4 indexed queries)", lat)


def fts_queries(file, prefix_last):
    out = []
    for line in list(open(f"{urls}/{file}"))[:n]:
        q = urllib.parse.parse_qs(urllib.parse.urlparse(line.strip()).query).get("q", [""])[0]
        toks = [t for t in re.split(r"\W+", q) if t]
        if not toks:
            continue
        parts = [f'"{t}"' for t in toks]
        if prefix_last and not q.endswith(" "):
            parts[-1] += "*"
        out.append(" AND ".join(parts))
    return out


for name, file, prefix in [("name_full (FTS5, top 20)", "name_full.txt", False), ("name_prefix_typeahead (FTS5, top 20)", "name_prefix_typeahead.txt", True)]:
    lat = []
    for m in fts_queries(file, prefix):
        t = time.perf_counter()
        db.execute("SELECT count(*) FROM nomes WHERE nomes MATCH ?", (m,)).fetchone()
        db.execute("SELECT rowid FROM nomes WHERE nomes MATCH ? ORDER BY rank LIMIT 20", (m,)).fetchall()
        lat.append((time.perf_counter() - t) * 1e3)
    report(name, lat)
