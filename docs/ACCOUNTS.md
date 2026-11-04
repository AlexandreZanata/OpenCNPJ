# Accounts, keys, quotas and the admin API

OpenCNPJ authenticates API users with a **single login key**. A user registers with a
username and an e-mail address only; the key is e-mailed to them. The key is the only
credential: it logs in, it can be sent on every request, and it can be revoked and replaced
through the e-mail address on file.

Everything lives in one embedded SQLite file (`<data-dir>/accounts.sqlite`, WAL mode). There
is no external database, cache server or paid service.

## Credentials

| Credential | Format | Stored as | Lifetime |
|---|---|---|---|
| Login key | `ocnpj_<10-char id>_<32-char secret>` (base62) | Argon2id PHC hash of the secret (m = 19 MiB, t = 2, p = 1) | until rotated or revoked |
| Session token | `ocs_<43-char secret>` | SHA-256 | `--session-hours` (default 168) |
| Recovery token | `ocr_<43-char secret>` | SHA-256 | 30 minutes, single use |

Send either credential as `Authorization: Bearer <key or token>`. `X-API-Key: <key>` is also
accepted. A user has at most one active key ("chave única"); issuing a new one revokes the old
one.

Argon2 runs once per key per hour: a verified key is remembered by its SHA-256, so the
hot path costs a hash and a map lookup, and on a restart the stopping process hands the
remembered digests to its successor (DEPLOYMENT.md). Revoking a key, or suspending or
banning a user, takes effect immediately in the server that does it; a key rotated from
the command line while a server runs stops working there within the hour (use the admin
API for immediate effect). Session tokens are SHA-256 lookups and never need Argon2.

## Account lifecycle

```
register (username, email) ──► pending ──first login with the e-mailed key──► active
                                                    │
        admin/moderator: suspend (reason, optional until) ──► suspended ──► reactivate
        admin/moderator: ban (reason) ──────────────────────► banned ────► reactivate (admin)
```

* `pending` accounts can log in (that is how the e-mail is proven); the first login marks the
  address as verified.
* `suspended` and `banned` accounts get `403 account_suspended` / `403 account_banned`;
  a suspension with `until` lifts itself at that time.

## Public endpoints (`/api/v1/auth`)

| Method and path | Body | Result |
|---|---|---|
| `POST /api/v1/auth/register` | `{"username","email"}` | `202` — the key is e-mailed. `409 username_taken`, `400 invalid_username` (3–32 of `a-z 0-9 . _ -`), `400 invalid_email`, `403 registration_closed`. An address that already has an account gets a "use recovery" e-mail instead; the response is the same, so addresses cannot be enumerated. |
| `POST /api/v1/auth/login` | `{"key"}` | `{"token","token_type":"Bearer","expires_at","user"}`. `401 invalid_key`. |
| `POST /api/v1/auth/logout` | — | `204`; the session token is revoked. |
| `GET /api/v1/auth/me` | — | `{"user","plan","limits","usage"}` (see below). |
| `POST /api/v1/auth/key/rotate` | — | `{"key","key_prefix"}`: the old key and every session are revoked; a notice is e-mailed. |
| `POST /api/v1/auth/recover` | `{"email"}` | Always `202`. If the address has an account, a confirmation link valid for 30 minutes is e-mailed. |
| `GET /api/v1/auth/recover/confirm?token=` | — | HTML page with a confirm button (a GET never changes anything, so mail scanners that open links are harmless). |
| `POST /api/v1/auth/recover/confirm` | `{"token"}` (JSON or form) | Revokes the old key and every session, issues a new key, e-mails it and returns it (`{"key","key_prefix"}`, or an HTML page for form posts). `400 invalid_token`. |

`GET /api/v1/auth/me`:

```json
{
  "user": { "id": 7, "username": "ana", "email": "ana@example.com", "role": "user", "status": "active", "plan": "free", "key_prefix": "Ab3dE5gH9k", ... },
  "plan": { "name": "free", "daily_limit": 1000, "monthly_limit": 20000, "per_minute": 60, "burst": 30, "max_page_size": 100, "export": false, "max_export_rows": 0, "downloads": false },
  "limits": { "daily_limit": 1000, "monthly_limit": 20000, "per_minute": 60 },
  "usage": { "today": 12, "month": 310 }
}
```

### Auth modes

`serve --auth required` (default with `--data-dir`) requires a credential on every data route.
`--auth optional` serves unauthenticated requests under the `anonymous` plan, limited per IP.
`--auth off` disables accounts entirely (the previous behaviour; used by benchmarks).

Always public: `/`, `/healthz`, `/health`, `/readyz`, `/api/v1/campos`, `/api/v1/dominios*`,
`/api/v1/snapshot`, `/api/v1/distribution` and `/api/v1/auth/*`. Data routes (`cnpj`,
`estabelecimentos`, `empresas`, `cpf`, `search`, `busca`, `stats`, `export`, `/downloads/*`)
need a credential. `/metrics` moves to the admin listener.

## Plans, quotas and rate limits

A plan sets: `daily_limit`, `monthly_limit` (requests; `null` = unlimited), `per_minute` and
`burst` (token bucket), `max_page_size`, `export` and `max_export_rows`, `downloads`.
A user can override `daily_limit`, `monthly_limit` and `per_minute`.

Seeded plans: `anonymous` (100/day, 10/min), `free` (1,000/day, 20,000/month, 60/min),
`pro` (100,000/day, 2,000,000/month, 600/min, export up to 100,000 rows) and `unlimited`
(staff default). Admins edit them freely.

Every data request counts as one search. It is admitted only when the bucket and both
counters allow it; a 5xx response is refunded. Rejections are `429 rate_limited` or
`429 quota_exceeded` with `Retry-After`. Every authenticated data response carries:

```
X-RateLimit-Limit: 60            X-RateLimit-Remaining: 59
X-Quota-Daily-Limit: 1000        X-Quota-Daily-Remaining: 988
X-Quota-Monthly-Limit: 20000     X-Quota-Monthly-Remaining: 19690
X-Cache: HIT | MISS
```

Days and months are UTC. Counters live in memory and are written to SQLite every five seconds
in one transaction (and on shutdown), as increments, so several processes sharing the file
stay correct.

## Roles and responsibilities

| Role | Responsibilities | Permissions |
|---|---|---|
| `admin` | Owns the service: accounts, roles, plans, settings, snapshots and jobs. | all |
| `moderator` | Handles abuse and support: suspends, bans, reactivates and rotates keys of `user` accounts, resets usage, writes notes. | `users.read`, `users.moderate`, `usage.read`, `audit.read`, `system.read` |
| `analyst` | Read-only operations and usage reporting. | `users.read`, `usage.read`, `audit.read`, `system.read` |
| `user` | API access under their plan. | — |

Permissions: `users.read`, `users.moderate`, `users.manage` (create, delete, e-mail, role,
plan, limits), `plans.manage`, `usage.read`, `audit.read`, `system.read` (overview, metrics,
snapshots, reports, index, explain, update status), `system.operate` (activate, reload, caches,
jobs, self-test, update check), `settings.manage`. Moderators act only on `user` accounts.
Nobody can change their own role or suspend themselves, and the last active admin cannot be
demoted, suspended or deleted. Every change made through the admin API is written to the audit
log with the actor, target, details and client address.

## Admin API (admin listener, `--admin-bind`)

All `/admin/api/*` routes require a staff credential (`Authorization: Bearer`) with the listed
permission; `401 unauthenticated` / `403 forbidden` otherwise. The admin listener also serves
the public API, including `/api/v1/auth/login`.

| Method and path | Permission | Body / query | Result |
|---|---|---|---|
| `GET /admin/api/whoami` | staff | — | `{"user","permissions":[...]}` |
| `GET /admin/api/roles` | staff | — | `[{"role","description","permissions"}]` |
| `GET /admin/api/users` | `users.read` | `q` (username/e-mail substring), `status`, `role`, `plan`, `sort` (`created`, `-created`, `username`, `usage_today`, `-usage_today`, `last_seen`), `limit` (≤ 500), `offset` | `{"total","users":[User + "usage_today","usage_month"]}` |
| `POST /admin/api/users` | `users.manage` | `{"username","email","role"?,"plan"?,"send_email"?}` | `{"user","key"}` (key shown once; account starts `active`) |
| `GET /admin/api/users/{id}` | `users.read` | — | `{"user","keys","sessions","usage_daily":[{"day","kind","count","cache_hits"}],"recent_searches":[...]}` |
| `PATCH /admin/api/users/{id}` | `users.manage` (`notes`: `users.moderate`) | any of `email`, `role`, `plan`, `daily_limit`, `monthly_limit`, `per_minute` (`null` clears an override), `notes` | `{"user"}` |
| `POST /admin/api/users/{id}/suspend` | `users.moderate` | `{"reason","until"?}` (`until` = unix seconds) | `{"user"}` |
| `POST /admin/api/users/{id}/ban` | `users.moderate` | `{"reason"}` | `{"user"}` |
| `POST /admin/api/users/{id}/reactivate` | `users.moderate` | — | `{"user"}` |
| `POST /admin/api/users/{id}/rotate-key` | `users.moderate` | `{"send_email"?}` | `{"key","key_prefix"}` |
| `POST /admin/api/users/{id}/revoke-sessions` | `users.moderate` | — | `{"revoked"}` |
| `POST /admin/api/users/{id}/reset-usage` | `users.moderate` | `{"scope":"day"\|"month"}` | `{"user"}` |
| `DELETE /admin/api/users/{id}` | `users.manage` | — | `{"deleted":true}` |
| `GET /admin/api/plans` | `users.read` | — | `[Plan + "users"]` |
| `POST /admin/api/plans` | `plans.manage` | Plan fields | `Plan` |
| `PATCH /admin/api/plans/{id}` | `plans.manage` | any Plan field | `Plan` |
| `DELETE /admin/api/plans/{id}` | `plans.manage` | — | `409` while users are on it |
| `GET /admin/api/usage` | `usage.read` | `days` (≤ 400, default 30) | `{"daily":[{"day","requests","users","cache_hits"}],"by_kind":{...},"top_users_today":[{"id","username","count"}],"totals":{"users","active_users_today","requests_today","requests_month"}}` |
| `GET /admin/api/searches` | `usage.read` | `user_id`, `limit` (≤ 1000), `before` (id) | `[{"id","ts","user_id","username","route","query","status","ms","cache"}]` |
| `GET /admin/api/audit` | `audit.read` | `actor_id`, `target_id`, `limit`, `before` | `[{"id","ts","actor_id","actor","action","target_id","target","details","ip"}]` |
| `GET /admin/api/settings` | `system.read` | — | Settings |
| `PATCH /admin/api/settings` | `settings.manage` | `registration` (`open`\|`closed`), `default_plan`, `search_log` (bool), `search_log_days`, `auto_update` (bool), `update_check_hours` | Settings |
| `GET /admin/api/updates` | `system.read` | — | `{"enabled","interval_hours","last_check","last_result","latest_remote","current_competence","next_check","job"}` |
| `POST /admin/api/updates/check` | `system.operate` | — | starts a check now |
| `GET /admin/api/response-cache` | `system.read` | — | `{"entries","bytes","budget","hits","misses","coalesced"}` |
| `DELETE /admin/api/response-cache` | `system.operate` | — | clears it |
| `GET /admin/api/overview`, `metrics`, `snapshots`, `index`, `reports`, `reports/{id}`, `jobs`, `jobs/{id}/log`, `explain` | `system.read` | as before | as before |
| `POST /admin/api/snapshots/activate`, `reload`, `cache` (POST/DELETE), `jobs` (POST), `jobs/{id}` (DELETE), `selftest` | `system.operate` | as before | as before |
| `GET /metrics` | `system.read` | — | Prometheus text |

`User` object:

```json
{ "id": 7, "username": "ana", "email": "ana@example.com", "role": "user", "status": "active",
  "plan": "free", "created_at": 1793750000, "verified_at": 1793750100, "last_seen_at": 1793760000,
  "suspended_until": null, "status_reason": null, "notes": "",
  "daily_limit": null, "monthly_limit": null, "per_minute": null, "key_prefix": "Ab3dE5gH9k" }
```

`Plan` object: `{"id","name","daily_limit","monthly_limit","per_minute","burst","max_page_size","export","max_export_rows","downloads"}`
(`null` limits mean unlimited).

## E-mail

`--smtp-url` (or `OPENCNPJ_SMTP_URL`), e.g. `smtps://user:password@smtp.example.com:465` or
`smtp://smtp.example.com:587?tls=required`, and `--mail-from` (`OPENCNPJ_MAIL_FROM`). Links in
e-mails use `--public-url`. Without SMTP, messages are written to `<data-dir>/outbox/*.eml`
and logged, which is enough for a private install and for tests.

## Command line

```bash
opencnpj accounts --data-dir DATA create --username admin --email admin@example.com --role admin
opencnpj accounts --data-dir DATA list
opencnpj accounts --data-dir DATA set-role --username ana --role moderator
opencnpj accounts --data-dir DATA rotate-key --username ana
```

```bash
opencnpj accounts --data-dir DATA backup --out accounts-2026-11-04.sqlite   # consistent copy while running
opencnpj accounts --data-dir DATA seed --count 5000 --plan pro --key-file keys.txt   # load-test accounts
```

`create` and `rotate-key` print the key once (or write it to `--key-file`, mode 0600).
These commands work on the SQLite file directly, with or without a running server.
