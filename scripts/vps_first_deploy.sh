#!/usr/bin/env bash
# VPS first deploy: Docker Postgres + Redis, restore CNPJ dump, API, nginx, credentials.
# Invoked by deploy_all_to_vps.sh — do not run alone without binaries + dump in /var/lib/opencnpj/incoming/.
set -euo pipefail

REPO="${REPO:-/opt/opencnpj}"
INCOMING="${INCOMING:-/var/lib/opencnpj/incoming}"
ETC="/etc/opencnpj"
DUMP_TAG="${DUMP_TAG:-$(date +%Y%m)}"
ADMIN_EMAIL="${ADMIN_EMAIL:-admin@comerc.app.br}"
PG_CONTAINER="${PG_CONTAINER:-opencnpj-postgres}"
REDIS_CONTAINER="${REDIS_CONTAINER:-opencnpj-redis}"
PG_PORT="${PG_PORT:-5435}"
REDIS_PORT="${REDIS_PORT:-6381}"

FINISH_RESTORE="${FINISH_RESTORE:-0}"

gen_pass() { openssl rand -base64 24 | tr -d '/+=' | head -c 24; }

log() { echo "[$(date '+%H:%M:%S')] $*"; }

validate_counts() {
  local db="$1"
  local estab
  estab="$(docker exec "$PG_CONTAINER" psql -U postgres -d "$db" -tAc "SELECT count(*) FROM estabelecimentos" | tr -d '[:space:]')"
  if [[ -z "$estab" || "$estab" -lt 20000000 ]]; then
    log "ERROR: restore incomplete — estabelecimentos=$estab (expected >= 20000000)"
    exit 1
  fi
  log "OK: estabelecimentos=$estab"
}

load_reference_tables() {
  local db="$1"
  log "Loading reference tables (RFB CSV) into $db…"
  local cfg="/tmp/opencnpj-import-config.yaml"
  cat >"$cfg" <<EOF
database:
  host: 127.0.0.1
  port: ${PG_PORT}
  migrate_port: ${PG_PORT}
  user: postgres
  password: opencnpj_super
  name: ${db}
  sslmode: disable
EOF
  CONFIG_FILE="$cfg" "$REPO/bin/opencnpj-importer" \
    --data-path="$REPO/data/refs" \
    --refs-only --no-clean --sample-percent=100
  docker exec "$PG_CONTAINER" psql -U postgres -d "$db" -tAc "
    SELECT 'naturezas='||count(*) FROM naturezas
    UNION ALL SELECT 'qualificacoes='||count(*) FROM qualificacoes
    UNION ALL SELECT 'paises='||count(*) FROM paises;"
}

reference_tables_loaded() {
  local db="$1"
  local naturezas
  naturezas="$(docker exec "$PG_CONTAINER" psql -U postgres -d "$db" -tAc "SELECT count(*) FROM naturezas" | tr -d '[:space:]')"
  [[ -n "$naturezas" && "$naturezas" -gt 0 ]]
}

create_search_indexes() {
  local db="$1"
  log "Creating search indexes (skipping FK constraints)…"
  local sql="$REPO/scripts/vps_create_indexes.sql"
  if [[ "${SKIP_ANALYZE:-0}" == "1" ]]; then
    grep -v '^ANALYZE ' "$sql" | docker exec -i "$PG_CONTAINER" psql -U postgres -d "$db" -v ON_ERROR_STOP=1
  else
    docker exec -i "$PG_CONTAINER" psql -U postgres -d "$db" -v ON_ERROR_STOP=1 <"$sql"
  fi
}

swap_to_production_db() {
  local staging="$1"
  # DROP DATABASE cannot run inside a transaction block — each statement needs its own -c.
  docker exec -i "$PG_CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 -c \
    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = 'opencnpj_cnpj' AND pid <> pg_backend_pid();"
  docker exec -i "$PG_CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS opencnpj_cnpj;"
  docker exec -i "$PG_CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 -c \
    "ALTER DATABASE ${staging} RENAME TO opencnpj_cnpj;"
  psql_super -d opencnpj_cnpj -c "
    GRANT CONNECT ON DATABASE opencnpj_cnpj TO opencnpj_reader;
    GRANT USAGE ON SCHEMA public TO opencnpj_reader;
    GRANT SELECT ON ALL TABLES IN SCHEMA public TO opencnpj_reader;
    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO opencnpj_reader;
    ANALYZE;
  "
}

ensure_docker() {
  command -v docker >/dev/null || { log "ERROR: docker missing"; exit 1; }
}

start_postgres() {
  if docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
    log "Postgres container already running"
    return
  fi
  docker rm -f "$PG_CONTAINER" 2>/dev/null || true
  docker volume rm opencnpj_pgdata 2>/dev/null || true
  docker volume create opencnpj_pgdata >/dev/null
  docker run -d --name "$PG_CONTAINER" \
    -e POSTGRES_PASSWORD=opencnpj_super \
    -v opencnpj_pgdata:/var/lib/postgresql \
    -p "127.0.0.1:${PG_PORT}:5432" \
    postgres:18.4-alpine
  for _ in $(seq 1 30); do
    docker exec "$PG_CONTAINER" pg_isready -U postgres >/dev/null 2>&1 && break
    sleep 2
  done
}

start_redis() {
  if docker ps --format '{{.Names}}' | grep -qx "$REDIS_CONTAINER"; then
    return
  fi
  docker rm -f "$REDIS_CONTAINER" 2>/dev/null || true
  docker run -d --name "$REDIS_CONTAINER" \
    -p "127.0.0.1:${REDIS_PORT}:6379" \
    redis:7-alpine redis-server --maxmemory 128mb --maxmemory-policy allkeys-lru
}

psql_super() {
  docker exec -i "$PG_CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 "$@"
}

bootstrap_db() {
  local reader saas migrate restore
  reader="$(gen_pass)"
  saas="$(gen_pass)"
  migrate="$(gen_pass)"
  restore="$(gen_pass)"

  psql_super <<SQL
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'opencnpj_reader') THEN
    CREATE ROLE opencnpj_reader LOGIN PASSWORD '${reader}';
  ELSE
    ALTER ROLE opencnpj_reader PASSWORD '${reader}';
  END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'opencnpj_saas') THEN
    CREATE ROLE opencnpj_saas LOGIN PASSWORD '${saas}';
  ELSE
    ALTER ROLE opencnpj_saas PASSWORD '${saas}';
  END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'opencnpj_migrate_saas') THEN
    CREATE ROLE opencnpj_migrate_saas LOGIN PASSWORD '${migrate}';
  ELSE
    ALTER ROLE opencnpj_migrate_saas PASSWORD '${migrate}';
  END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'opencnpj_restore') THEN
    CREATE ROLE opencnpj_restore LOGIN PASSWORD '${restore}';
  ELSE
    ALTER ROLE opencnpj_restore PASSWORD '${restore}';
  END IF;
END \$\$;
SELECT 'CREATE DATABASE opencnpj_cnpj' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'opencnpj_cnpj')\gexec
SELECT 'CREATE DATABASE opencnpj_saas OWNER opencnpj_saas' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'opencnpj_saas')\gexec
SQL

  psql_super -d opencnpj_cnpj -c "GRANT CONNECT ON DATABASE opencnpj_cnpj TO opencnpj_reader, opencnpj_restore;"
  psql_super -d opencnpj_cnpj -c "GRANT ALL ON SCHEMA public TO opencnpj_restore; GRANT USAGE ON SCHEMA public TO opencnpj_reader;"

  echo "$reader" > /tmp/.opencnpj_reader
  echo "$saas" > /tmp/.opencnpj_saas
}

restore_dump() {
  local archive="${INCOMING}/opencnpj_cnpj_${DUMP_TAG}.dump.zst"
  local dump="${INCOMING}/opencnpj_cnpj_${DUMP_TAG}.dump"
  local db="opencnpj_cnpj_new"

  if [[ "${FINISH_RESTORE}" == "1" ]]; then
    log "FINISH_RESTORE=1 — skipping pg_restore (data already loaded)"
    validate_counts "$db"
    if reference_tables_loaded "$db"; then
      log "Reference tables already loaded — skipping"
    else
      load_reference_tables "$db"
    fi
    SKIP_ANALYZE=1 create_search_indexes "$db"
    swap_to_production_db "$db"
    return
  fi

  if [[ ! -f "$archive" && ! -f "$dump" ]]; then
    log "ERROR: no dump in $INCOMING (expected opencnpj_cnpj_${DUMP_TAG}.dump.zst)"
    exit 1
  fi
  [[ -f "$dump" ]] || zstd -d "$archive" -o "$dump"
  log "Restoring dump (may take 1–3 hours)…"
  docker exec -i "$PG_CONTAINER" psql -U postgres -c "DROP DATABASE IF EXISTS ${db};"
  docker exec -i "$PG_CONTAINER" psql -U postgres -c "CREATE DATABASE ${db};"
  docker cp "$dump" "${PG_CONTAINER}:/tmp/opencnpj_restore.dump"
  local dump_in="/tmp/opencnpj_restore.dump"
  log "pg_restore pre-data (schema)…"
  docker exec "$PG_CONTAINER" pg_restore --section=pre-data --no-owner --no-acl \
    -U postgres -d "$db" "$dump_in"
  log "pg_restore data (parallel, FK checks off)…"
  docker exec -e PGOPTIONS="-c session_replication_role=replica" "$PG_CONTAINER" \
    pg_restore --section=data -j 4 --no-owner --no-acl \
    -U postgres -d "$db" "$dump_in"
  docker exec "$PG_CONTAINER" rm -f "$dump_in"
  validate_counts "$db"
  load_reference_tables "$db"
  create_search_indexes "$db"
  swap_to_production_db "$db"
}

install_api() {
  local reader saas mfa admin_pass
  reader="$(cat /tmp/.opencnpj_reader)"
  saas="$(cat /tmp/.opencnpj_saas)"
  mfa="$(openssl rand -base64 32)"
  admin_pass="$(gen_pass)$(gen_pass | head -c 4)"

  id -u opencnpj &>/dev/null || useradd -r -m -d "$REPO" -s /bin/bash opencnpj
  mkdir -p "$ETC" /var/log/opencnpj
  chmod 700 "$ETC"

  openssl genrsa -out "$ETC/jwt-private.pem" 2048 2>/dev/null
  openssl rsa -in "$ETC/jwt-private.pem" -pubout -out "$ETC/jwt-public.pem" 2>/dev/null
  chmod 600 "$ETC/jwt-private.pem" "$ETC/jwt-public.pem"

  cp "$REPO/config/config.saas.example.yaml" "$ETC/config.saas.yaml"
  sed -i "s/CHANGE_ME/${saas}/g" "$ETC/config.saas.yaml" || true

  cat >"$ETC/api.env" <<EOF
CONFIG_FILE=$ETC/config.saas.yaml
CNPJ_DATABASE_URL=postgres://opencnpj_reader:${reader}@127.0.0.1:${PG_PORT}/opencnpj_cnpj?sslmode=disable
SAAS_DATABASE_URL=postgres://opencnpj_saas:${saas}@127.0.0.1:${PG_PORT}/opencnpj_saas?sslmode=disable
REDIS_URL=redis://127.0.0.1:${REDIS_PORT}/0
MFA_SECRET_ENCRYPTION_KEY=${mfa}
ADMIN_JWT_PRIVATE_KEY_PATH=$ETC/jwt-private.pem
ADMIN_JWT_PUBLIC_KEY_PATH=$ETC/jwt-public.pem
REFRESH_TOKEN_COOKIE_NAME=opencnpj_admin_refresh
EOF
  chmod 600 "$ETC/api.env"

  install -m 755 "$REPO/bin/opencnpj-migrate" /usr/local/bin/opencnpj-migrate
  install -m 755 "$REPO/bin/opencnpj-api" /usr/local/bin/opencnpj-api
  install -m 755 "$REPO/bin/opencnpj-seed" /usr/local/bin/opencnpj-seed

  set -a; source "$ETC/api.env"; set +a
  (cd "$REPO" && CONFIG_FILE="$ETC/config.saas.yaml" /usr/local/bin/opencnpj-migrate --saas)

  (
    cd "$REPO"
    ADMIN_EMAIL="$ADMIN_EMAIL" ADMIN_PASSWORD="$admin_pass" CONFIG_FILE="$ETC/config.saas.yaml" \
      /usr/local/bin/opencnpj-seed
  ) >"$ETC/credentials.seed"

  cat >"$ETC/credentials.txt" <<EOF
# OpenCNPJ credentials — $(date -Iseconds) — KEEP SECRET
ADMIN_URL=https://admin.comerc.app.br/admin/login
API_URL=https://api.comerc.app.br/api/v1/cnpj/{cnpj}
ADMIN_EMAIL=$ADMIN_EMAIL
ADMIN_PASSWORD=$admin_pass
$(grep -E '^(TOTP_URL|TOTP_SECRET|API_KEY)=' "$ETC/credentials.seed" 2>/dev/null || true)
CNPJ_DATABASE_URL=postgres://opencnpj_reader:***@127.0.0.1:${PG_PORT}/opencnpj_cnpj
SAAS_DATABASE_URL=postgres://opencnpj_saas:***@127.0.0.1:${PG_PORT}/opencnpj_saas
EOF
  chmod 600 "$ETC/credentials.txt"
  chown -R opencnpj:opencnpj "$ETC" "$REPO" /var/log/opencnpj 2>/dev/null || true
}

install_systemd() {
  cat >/etc/systemd/system/opencnpj-api.service <<EOF
[Unit]
Description=OpenCNPJ SaaS API
After=docker.service network-online.target
Wants=network-online.target

[Service]
Type=simple
User=opencnpj
Group=opencnpj
WorkingDirectory=$REPO
EnvironmentFile=$ETC/api.env
ExecStart=/usr/local/bin/opencnpj-api
Restart=on-failure
RestartSec=5
MemoryMax=512M

[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload
  systemctl enable --now opencnpj-api
}

install_nginx() {
  cp "$REPO/deploy/saas/cloudflare-real-ip.conf.example" /etc/nginx/snippets/cloudflare-real-ip.conf
  cp "$REPO/deploy/saas/nginx-comerc.app.br.example" /etc/nginx/sites-available/opencnpj-comerc
  sed -i 's|/etc/letsencrypt/live/admin.comerc.app.br/|/etc/letsencrypt/live/api.comerc.app.br/|g' \
    /etc/nginx/sites-available/opencnpj-comerc
  ln -sf /etc/nginx/sites-available/opencnpj-comerc /etc/nginx/sites-enabled/
  if [[ -f /etc/letsencrypt/live/api.comerc.app.br/fullchain.pem ]]; then
    log "TLS cert already present — skipping certbot"
  else
    certbot certonly --nginx -d api.comerc.app.br -d admin.comerc.app.br --non-interactive --agree-tos -m "$ADMIN_EMAIL" 2>/dev/null \
      || certbot certonly --standalone -d api.comerc.app.br -d admin.comerc.app.br --non-interactive --agree-tos -m "$ADMIN_EMAIL" \
      || log "WARN: certbot failed — run manually"
  fi
  nginx -t && systemctl reload nginx
}

main() {
  if [[ "${API_ONLY:-0}" == "1" ]]; then
    ensure_docker
    start_postgres
    start_redis
    bootstrap_db
    install_api
    install_systemd
    install_nginx
    sleep 3
    curl -sf http://127.0.0.1:8081/readyz && log "API ready" || log "WARN: /readyz not 200 yet"
    log "=== CREDENTIALS (also in $ETC/credentials.txt) ==="
    cat "$ETC/credentials.txt"
    return
  fi
  ensure_docker
  start_postgres
  start_redis
  bootstrap_db
  restore_dump
  install_api
  install_systemd
  install_nginx
  sleep 3
  curl -sf http://127.0.0.1:8081/readyz && log "API ready" || log "WARN: /readyz not 200 yet"
  log "=== CREDENTIALS (also in $ETC/credentials.txt) ==="
  cat "$ETC/credentials.txt"
}

main "$@"
