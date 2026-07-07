.PHONY: build build-downloader test test-integration bench lint vet migrate import seed coverage setup download download-latest download-and-import import-full list-months web-dev web-build web-test sqlc sqlc-vet sqlc-install

GO      = go
SQLC    ?= sqlc
SQLC_VERSION = v1.29.0
BINARY  = bin/importer
DOWNLOADER = bin/downloader
GOFLAGS = -ldflags="-s -w"
DATA_DIR ?= ./data

build:
	$(GO) build $(GOFLAGS) -o $(BINARY) ./cmd/importer/

build-downloader:
	$(GO) build $(GOFLAGS) -o $(DOWNLOADER) ./cmd/downloader/

setup:
	bash scripts/setup_project.sh

download:
	bash scripts/download_latest.sh

download-latest:
	bash scripts/download_latest.sh

download-and-import:
	bash scripts/download_and_import.sh

import-full:
	bash scripts/run_full_import.sh

list-months:
	$(GO) run ./cmd/downloader --list

migrate:
	$(GO) run ./cmd/migrate

import-sample:
	bash scripts/import_sample.sh

benchmark-10pct:
	bash scripts/benchmark_import_10pct.sh

benchmark-20pct:
	SAMPLE_PERCENT=20 TARGET_SEC=360 bash scripts/benchmark_import_sample.sh

benchmark-all-approaches:
	bash scripts/benchmark_all_approaches.sh

guard-status:
	bash scripts/system_guard.sh status

guard-watch:
	@echo "usage: bash scripts/run_with_guard.sh <command>"

test:
	$(GO) test ./... -short -race -count=1

test-integration:
	$(GO) test ./tests/integration/... -v -timeout 15m

sqlc:
	$(SQLC) generate

sqlc-vet:
	$(SQLC) vet

sqlc-install:
	$(GO) install github.com/sqlc-dev/sqlc/cmd/sqlc@$(SQLC_VERSION)

bench:
	$(GO) test ./tests/benchmark/... -bench=. -benchmem -benchtime=5s -count=3 \
	  | tee /tmp/bench_$(shell date +%Y%m%d_%H%M%S).txt

lint:
	golangci-lint run --timeout 5m

vet:
	$(GO) vet ./...

seed:
	bash scripts/seed_test_fixtures.sh

import:
	$(BINARY) --dir=$(DATA_DIR) --db=$(DATABASE_URL) --workers=$(WORKERS)

coverage:
	$(GO) test ./... -short -coverprofile=coverage.out
	$(GO) tool cover -html=coverage.out -o coverage.html

web-dev:
	cd web && pnpm dev

web-build:
	cd web && pnpm install --frozen-lockfile && pnpm build

web-test:
	cd web && pnpm install --frozen-lockfile && pnpm test
