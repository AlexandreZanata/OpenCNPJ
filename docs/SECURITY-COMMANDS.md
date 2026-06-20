# Security Tooling Commands

Referência completa de todos os comandos usados no pipeline de segurança (CI e local).

---

## 1. Executar a stack completa localmente

Roda em sequência: golangci-lint → gosec → staticcheck → govulncheck.  
Instala as ferramentas automaticamente se não estiverem no PATH.

```bash
./scripts/security-check.sh
```

**Requisito:** estar na raiz do projeto (onde está o `go.mod`).  
**Saída:** exit 0 se tudo passou, exit 1 se algum check falhou.

---

## 2. Instalação das ferramentas (Go)

Cada comando instala a ferramenta em `$(go env GOPATH)/bin`. Garanta que esse diretório está no `PATH`.

| Ferramenta        | Comando de instalação |
|-------------------|------------------------|
| golangci-lint     | `go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest` |
| gosec             | `go install github.com/securego/gosec/v2/cmd/gosec@latest` |
| staticcheck       | `go install honnef.co/go/tools/cmd/staticcheck@latest` |
| govulncheck       | `go install golang.org/x/vuln/cmd/govulncheck@latest` |
| nancy             | `go install github.com/sonatype-nexus-community/nancy@latest` |

**Exemplo — adicionar GOPATH/bin ao PATH e instalar tudo:**

```bash
export PATH="${PATH}:$(go env GOPATH)/bin"

go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest
go install github.com/securego/gosec/v2/cmd/gosec@latest
go install honnef.co/go/tools/cmd/staticcheck@latest
go install golang.org/x/vuln/cmd/govulncheck@latest
go install github.com/sonatype-nexus-community/nancy@latest
```

---

## 3. Comandos de análise (rodar manualmente)

Execute na **raiz do projeto** (onde estão `go.mod` e `.golangci.yml`).

### 3.1 golangci-lint

Lint com gosec, staticcheck, errcheck, govet, ineffassign, unused (config em `.golangci.yml`).

```bash
golangci-lint run --timeout=5m --config=.golangci.yml ./...
```

### 3.2 gosec (SAST)

**Somente saída no terminal:**

```bash
gosec ./...
```

**Gerar SARIF (para upload no GitHub Security):**

```bash
gosec -fmt sarif -out gosec-results.sarif ./...
```

### 3.3 staticcheck

```bash
staticcheck ./...
```

### 3.4 govulncheck (dependências)

```bash
govulncheck ./...
```

**Modo verboso (mais detalhes):**

```bash
govulncheck -show verbose ./...
```

### 3.5 nancy (dependências — Sonatype OSS Index)

Lista de módulos em JSON piped para o nancy:

```bash
go list -json -m all | nancy sleuth
```

**Alternativa usando arquivo (se o pipe não for suportado):**

```bash
go list -json -m all > go.list
nancy sleuth -p go.list
```

---

## 4. Comandos que rodam no CI (GitHub Actions)

O workflow `.github/workflows/security.yml` executa os mesmos comandos nos seguintes jobs.

### Job: lint

- **Checkout:** `actions/checkout@v4`
- **Go:** `actions/setup-go@v5` com `go-version-file: go.mod`
- **Cache:** `actions/cache@v4` (chave: `go.sum`)
- **Lint:** `golangci-lint run --timeout=5m --config=.golangci.yml` (via `golangci/golangci-lint-action@v6`)

### Job: sast (depende de lint)

- **Instalar gosec:**  
  `go install github.com/securego/gosec/v2/cmd/gosec@latest`
- **Instalar staticcheck:**  
  `go install honnef.co/go/tools/cmd/staticcheck@latest`
- **Rodar gosec (SARIF):**  
  `gosec -fmt sarif -out gosec-results.sarif ./...`
- **Rodar staticcheck:**  
  `staticcheck ./...`
- **Upload SARIF:** ação `github/codeql-action/upload-sarif@v3` com `sarif_file: gosec-results.sarif` e `if: always()`

### Job: dependency-scan (depende de sast)

- **Instalar govulncheck:**  
  `go install golang.org/x/vuln/cmd/govulncheck@latest`
- **Rodar govulncheck:**  
  `govulncheck ./...`
- **Instalar nancy:**  
  `go install github.com/sonatype-nexus-community/nancy@latest`
- **Rodar nancy:**  
  `go list -json -m all | nancy sleuth`

---

## 5. Resumo em uma linha (local)

Para quem já tem as ferramentas instaladas e só quer repetir os 4 checks na ordem do script:

```bash
golangci-lint run --timeout=5m --config=.golangci.yml ./... && \
gosec ./... && \
staticcheck ./... && \
govulncheck ./...
```

Para incluir nancy (como no CI):

```bash
golangci-lint run --timeout=5m --config=.golangci.yml ./... && \
gosec ./... && \
staticcheck ./... && \
govulncheck ./... && \
go list -json -m all | nancy sleuth
```

---

## 6. Arquivos de configuração relacionados

| Arquivo            | Uso |
|--------------------|-----|
| `.golangci.yml`    | Config do golangci-lint (linters, gosec severity/confidence, timeout 5m). |
| `.github/workflows/security.yml` | Pipeline Security no GitHub Actions. |
| `scripts/security-check.sh`      | Script local que instala (se necessário) e roda os 4 checks. |

Ver também: [SECURITY.md](SECURITY.md) para política de severidade, supressões e como reportar vulnerabilidades.

---

## 7. Qualidade de código (pacote completo)

Sequência recomendada para validar qualidade localmente, na mesma ordem da CI:

```bash
# 0) Dependências e organização
go mod tidy

# 1) Formatação e imports
gofmt -w .
goimports -w .

# 2) Build e checks básicos
go vet ./...
go test ./... -short -race -count=1

# 3) Lint completo
golangci-lint run --timeout 5m

# 4) Segurança e análise estática
gosec ./...
staticcheck ./...
govulncheck ./...
go list -json -m all | nancy sleuth

# 5) Testes de integração
go test ./tests/integration/... -v -timeout 15m

# 6) Benchmarks
go test ./tests/benchmark/... -bench=. -benchmem -benchtime=5s -count=3
```

Atalhos por `Makefile`:

```bash
make build
make lint
make test
make test-integration
make bench
make coverage
```

Validação de commit convencional (local):

```bash
npm install --save-dev @commitlint/cli @commitlint/config-conventional
npx commitlint --from=HEAD~1 --to=HEAD --verbose
```
