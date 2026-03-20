.PHONY: build test test-integration bench lint vet migrate import seed coverage

GO      = go
BINARY  = bin/importer
GOFLAGS = -ldflags="-s -w"

build:
	$(GO) build $(GOFLAGS) -o $(BINARY) ./cmd/importer/

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
