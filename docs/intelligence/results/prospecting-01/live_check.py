#!/usr/bin/env python3
"""Live check of the product server: decisions, filters and server-side time per question.

Usage: live_check.py BASE_URL KEY_FILE
  BASE_URL  e.g. http://127.0.0.1:8080 (a server started with --assistant-agg)
  KEY_FILE  file holding a login key of that server (never printed)

Each question is asked twice; the second timing is reported (warm caches). `server ms` is
`timing_us.total` of the reply; `wall ms` includes HTTP, authentication and the quota gate.
"""

import json
import sys
import time
import urllib.request

QUESTIONS = [
    ("dentistas em Cuiabá/MT com telefone", "execute"),
    ("novas empresas em Sinop/MT", "execute_with_note"),
    ("academias abertas em 2024 em Goiânia/GO", "execute"),
    ("telefones de imobiliárias em Londrina/PR", "execute"),
    ("restaurantes em Sinop/MT com telefone e capital acima de 50 mil", "execute"),
    ("liste os maiores restaurantes de Goiânia/GO", "execute_with_note"),
    ("supermercados em Sinop e Sorriso", "execute_with_note"),
    ("Quantas farmácias ativas existem em Sinop/MT?", "execute"),
    ("Quais as 10 cidades de MT com mais padarias ativas?", "execute"),
    ("33.000.167/0001-01", "execute"),
    ("filiais do Bradesco em MT", "execute"),
    ("empresas de energia solar em Sinop", "clarify"),
    ("liste as farmácias de Sinop/MT com DDD 66", "clarify"),
    ("preciso prospectar clientes para minha distribuidora de bebidas em Cuiabá", "clarify"),
    ("sócios da empresa Zaflas", "reject"),
]


def ask(base, key, question):
    request = urllib.request.Request(
        base + "/api/v1/assistant/ask",
        data=json.dumps({"question": question}).encode(),
        headers={"Content-Type": "application/json", "Authorization": "Bearer " + key},
    )
    started = time.perf_counter()
    reply = json.load(urllib.request.urlopen(request, timeout=60))
    return reply, (time.perf_counter() - started) * 1000


def main():
    base, key = sys.argv[1].rstrip("/"), open(sys.argv[2], encoding="utf-8").read().strip()
    wrong = 0
    print(f"{'decision':18} {'server ms':>9} {'wall ms':>8}  question -> filters")
    for question, expected in QUESTIONS:
        ask(base, key, question)
        reply, wall = ask(base, key, question)
        answer = reply["answer"]
        ok = answer["decision"] == expected
        wrong += not ok
        server = reply["timing_us"].get("total", 0) / 1000
        mark = "" if ok else f"  EXPECTED {expected}"
        print(f"{answer['decision']:18} {server:9.2f} {wall:8.2f}  {question} -> {answer.get('filters', [])}{mark}")
    print(f"{len(QUESTIONS)} questions, {wrong} unexpected decisions")
    sys.exit(1 if wrong else 0)


if __name__ == "__main__":
    main()
