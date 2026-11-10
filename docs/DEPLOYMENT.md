# Deployment: thousands of simultaneous users on one machine

OpenCNPJ is one binary with an mmap'd snapshot file and an embedded SQLite accounts
database. There is no database server, cache server or queue to run or pay for. A single
VPS serves thousands of concurrent authenticated users; the measurements behind every
number below are in [BENCHMARKS.md](BENCHMARKS.md#multi-user-load-accounts-on).

```
 clients ──HTTPS──► Caddy (TLS, HTTP/2, retries) ──► opencnpj@blue ─┐  same ports
                                                    opencnpj@green ┘  (SO_REUSEPORT)
                                                         │
                         /var/lib/opencnpj: current → snapshots/2026-10-balanced.ocnpj (mmap)
                                            accounts.sqlite (WAL), outbox/, jobs/, raw/
```

## Sizing

| Resource | Recommendation | Why |
|---|---|---|
| CPU | 8–16 cores | CNPJ lookups are sub-millisecond; nationwide name searches cost about 60 ms of CPU each, aggregate stats more. The response cache absorbs repeated queries. |
| RAM | 16 GB (8 GB works) | The 8.3 GB snapshot is memory-mapped: the page cache keeps the hot part. The process itself used 220 MB anonymous memory under 5,000 users. |
| Disk | NVMe, 60 GB | Current + previous snapshot (2 × 8.3 GB), one release of source ZIPs during an update (~7 GB), build scratch (~15 GB). |
| Network | Any | Responses are compressed (gzip/br/zstd); a cached lookup is ~1–3 KB on the wire. |

Measured on one 16-core / 31 GB machine with the load generator on the same host (so the
server had less than the whole CPU):

* 5,000 simultaneous users, each with its own key and connection, one request per second
  each: ~5,000 req/s at p50 0.2 ms and p99 0.4–0.6 ms in steady state, 0 errors.
* 2,000 users without pauses: 64,948 req/s, p50 11 ms, p99 78 ms (99% response-cache hits).
* Without the response cache the engine serves ~34,600 CNPJ lookups/s, ~1,300 selective
  filter queries/s, ~800 CPF lookups/s and ~170–300 nationwide name searches/s. That last
  number is the real limit for traffic that never repeats a query: put the expensive work
  behind plans (rate limits, quotas) and let the cache handle what repeats.

## Install

```bash
sudo useradd --system --home /var/lib/opencnpj opencnpj
sudo install -d -o opencnpj -g opencnpj /var/lib/opencnpj
sudo install -m 0755 opencnpj /usr/local/bin/opencnpj
sudo install -d -m 0700 /etc/opencnpj && sudo cp deploy/env.example /etc/opencnpj/env   # then edit
sudo cp deploy/sysctl-opencnpj.conf /etc/sysctl.d/90-opencnpj.conf && sudo sysctl --system
sudo cp deploy/opencnpj@.service /etc/systemd/system/

# First data and the first admin.
sudo -u opencnpj opencnpj update --data-dir /var/lib/opencnpj
sudo -u opencnpj opencnpj accounts --data-dir /var/lib/opencnpj create \
    --username admin --email you@example.com --role admin --key-file /var/lib/opencnpj/admin.key

sudo systemctl enable --now opencnpj@blue
sudo cp deploy/Caddyfile /etc/caddy/Caddyfile && sudo systemctl reload caddy   # edit host names first
```

To serve the web application and the assistant, add to the service's `serve` flags
`--web /usr/local/share/opencnpj/web` (the contents of `web/dist`) and
`--assistant-agg /var/lib/opencnpj/intelligence/agg/<competence>.ocagg`, built from the
snapshot in `current` (`opencnpj-intel agg build`), with `--assistant-catalog` and
`--assistant-sectors` pointing at the installed catalogues and `--assistant-anchor` at the
data date. The assistant loads once at start and binds to the snapshot served then:
after a monthly update, build the new aggregates and restart (blue/green, below).

The admin listener (`127.0.0.1:8081`) is never exposed directly: reach it through an SSH
tunnel (`ssh -L 8081:127.0.0.1:8081 server`) or the IP-restricted Caddy block, from the
web dashboard (`/admin`) or the desktop app (`opencnpj-admin`).

## Zero downtime

There are three kinds of change, and none of them interrupts service:

1. **New RFB release** (monthly). The server checks the RFB share every
   `update_check_hours` (admin setting, default 6). When the release differs from the served
   snapshot (a newer competence, or files republished with other sizes), it runs
   `opencnpj update` as a child process at idle CPU and I/O priority: download, build,
   verify every block, then an atomic rename of `current`. The reload loop opens the new
   file and swaps an `Arc`; in-flight requests finish on the old snapshot, new ones use
   the new one, and the response cache moves to the new snapshot's keys. `previous` keeps
   the old file for `opencnpj rollback`. Status and a "check now" button are in the admin
   API (`/admin/api/updates`) and both admin UIs.
2. **Snapshot switch by hand**: `POST /admin/api/snapshots/activate` or `rollback`, same
   mechanism.
3. **New binary**: `sudo deploy/upgrade.sh ./opencnpj`. It starts the other colour with
   `--reuse-port`, so both processes accept on the same ports, then stops the old one.
   On SIGTERM the old process:
   1. writes its verified keys (SHA-256 of 190-bit random keys; revocation is re-checked
      by the importer) and its cached responses to the data directory;
   2. answers with `Connection: close` for `--drain-secs` (5 s), so keep-alive clients
      reconnect, landing on the new process;
   3. drains in-flight requests and writes its pending usage counters.

   The new process imports both files within 250 ms, so moving clients skip Argon2 and
   find a warm cache. Measured with 5,000 users: 0 failed requests, worst p99 of any
   second 9.2 ms during the switch, back to 0.4 ms one second later.

A cold boot without a predecessor (first start, or a reboot more than a day after the last
shutdown) verifies each key once with Argon2id (19 MiB, about 30–70 ms of CPU under load)
and starts with an empty response cache: with 5,000 users arriving in the same second the
first requests waited up to 22 s. Clients that log in once and use session tokens never
pay Argon2 on a new process.

## Cost levers

* **Plans** decide what each user may cost you: rate limit, daily and monthly quotas, page
  size, export rights and size, downloads. The `free` default (1,000 requests a day, 60 per
  minute, pages of 100, no export) bounds a single user's worst case at about 4 CPU-seconds
  per minute (60 nationwide name searches at ~60 ms each).
* **Response cache**: `--response-cache-mib` (default 256). Hits cost microseconds; the
  admin dashboard shows hit, coalesced and 304 counts. Entries expire after
  `--response-cache-ttl-secs` and whenever the snapshot changes.
* **Postings cache**: `--postings-cache-mib` (default 512) keeps decoded index lists for
  queries that miss the response cache.
* **Lanes**: lookups have their own concurrency bound, so a burst of heavy searches never
  delays them; heavy work waits up to `--timeout-ms` for a slot, then gets `503
  overloaded`.
* **Compression** is negotiated per request at the fastest level; snapshot downloads are
  never compressed and support `Range`.

## Operations checklist

* `opencnpj accounts --data-dir /var/lib/opencnpj backup --out /backup/accounts-$(date +%F).sqlite`
  daily (consistent while running); keep the e-mail outbox and `jobs/` logs only as long
  as needed.
* Scrape `/metrics` on the admin listener (staff key) with Prometheus: latency histograms
  per route, response-cache hits, rate-limit and quota rejections, RSS.
* Watch `/admin/api/updates` after each monthly release; `opencnpj rollback` reverts.
* SMTP: any provider or your own server through `OPENCNPJ_SMTP_URL`. Without it, e-mails
  land in `outbox/` and registration and recovery cannot reach users.
