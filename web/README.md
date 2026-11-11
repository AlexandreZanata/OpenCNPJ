# OpenCNPJ Web

Front end of the OpenCNPJ platform: commercial research and prospecting over the official
CNPJ data, in the browser. TypeScript 7 compiled to plain ES modules, HTML and CSS, with
**zero runtime dependencies** (no framework, no bundler). The compiled `dist/` is served by
`opencnpj serve --web web/dist`.

## What it does

| Page | Route | Purpose |
|---|---|---|
| Assistente | `#/` | Conversation with the deterministic assistant: counts, lists with CNPJ, rankings, series, comparisons and Excel/CSV/TXT files, with follow-ups. Each answer shows the filters it applied, how it was computed and its source |
| Busca | `#/busca/<filters>` | Search by filters (name, CNAE, state, city, status, size, regime, head office, contacts, opening date, capital) with pagination and CSV export. The filters live in the address, so a search can be bookmarked |
| CNPJ | `#/cnpj/<cnpj>` | Registration record of an establishment or of a company root; the same record opens as a side panel from any CNPJ in the other pages |
| Conta | `#/conta` | Plan and limits, day and month meters, thirty days of usage, login key rotation, API quick start, recent searches |
| Administração | `#/admin` | For staff, on the admin listener: accounts and per-account search limits, plans, service-wide usage |
| Sign-in | — | Login with the key, account creation and key recovery by e-mail (when the server runs with accounts) |

The application asks `GET /api/v1/platform` what the server offers and adapts: without
accounts (`--auth off`) there is no sign-in; without the assistant the first page points to
the search.

## Layout

```
web/
├── public/                 # copied as is
│   ├── index.html
│   ├── favicon.svg
│   ├── fonts/              # self-hosted Inter and Source Serif 4 (WOFF2)
│   └── styles/
│       ├── tokens.css      # every colour, radius and shadow; light and dark schemes
│       ├── base.css        # reset and element defaults
│       ├── app.css         # frame, pages and shared pieces
│       └── charts.css      # line and bar charts of assistant answers
├── src/
│   ├── main.ts             # bootstrap: platform, session, routes
│   ├── config.ts
│   ├── core/               # dom (h/s builders), api (client, session, quota), router, format
│   ├── layout/             # top bar and logo
│   ├── ui/                 # buttons, fields, tables, overlays, toasts, icons
│   ├── features/chat/      # answer blocks, charts, examples, reply types
│   ├── features/company/   # registration record (page and side panel)
│   └── pages/              # assistant, search, cnpj, account, admin, login
├── scripts/                # build.sh, tsc.sh (TypeScript 7.0.2), serve.sh
└── dist/                   # build output served by the API
```

Conventions: components are functions that return DOM elements (`core/dom.ts`); colours
come only from `tokens.css`; the session token is kept in `localStorage` and sent as
`Authorization: Bearer`; every request goes through `core/api.ts`, which also reads the
`X-Quota-*` headers so the usage in the top bar is always current.

## Build and run

```sh
sh web/scripts/build.sh            # type-checks and compiles src/ to dist/js, copies public/
cargo build --release -p opencnpj
./target/release/opencnpj serve --data-dir "$DATA" --web web/dist --auth trial \
  --assistant-agg "$DATA/intelligence/agg/2026-10.ocagg" \
  --assistant-history "$DATA/intelligence/backfill/aggregates"
```

Open `http://127.0.0.1:8080/` to try three searches. Create a free account or sign in
with an existing key to keep searching and save assistant conversations. The top bar
switches between light and dark themes and remembers the selection. The admin
listener (`http://127.0.0.1:8081/`) serves the same application with the administration
page for staff accounts; the operations dashboard stays at `/admin`.

`sh web/scripts/tsc.sh --noEmit` type-checks only.

## Local chat service

`opencnpj-intel serve --web web/dist` (crate `opencnpj-lab`) serves the assistant alone,
without accounts, on loopback or one LAN subnet, for development and evaluation. It
answers the product paths of the assistant (`/api/v1/assistant/*`, `/api/v1/platform`), so
this front end works there with the Busca and CNPJ tools hidden. For the whole product
without accounts use `opencnpj serve --auth off`.
