.PHONY: build build-downloader test test-integration bench lint vet migrate import seed coverage setup download list-months

GO      = go
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
	bash scripts/download_data.sh

list-months:
	$(GO) run ./cmd/downloader --list

test:
	$(GO) test ./... -short -race -count=1

test-integration:
	$(GO) test ./tests/integration/... -v -timeout 15m

bench:
	$(GO) test ./tests/benchmark/... -bench=. -benchmem -benchtime=5s -count=3 \
	  | tee /tmp/bench_$(shell date +%Y%m%d_%H%M%S).txt

lint:
	golangci-lint run --timeout 5m

vet:
	$(GO) vet ./...

migrate:
	psql $(DATABASE_URL) -f scripts/setup_db.sh

seed:
	bash scripts/seed_test_fixtures.sh

import:
	$(BINARY) --dir=$(DATA_DIR) --db=$(DATABASE_URL) --workers=$(WORKERS)

coverage:
	$(GO) test ./... -short -coverprofile=coverage.out
	$(GO) tool cover -html=coverage.out -o coverage.html
