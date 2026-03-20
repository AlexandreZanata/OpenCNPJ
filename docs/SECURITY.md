# Política de Segurança

Este documento descreve as ferramentas de segurança do projeto, como interpretar resultados, políticas de severidade e como reportar vulnerabilidades.

---

## 1. Ferramentas e o que cada uma detecta

| Ferramenta      | Tipo            | O que encontra                                                                 | Quando roda        |
|-----------------|-----------------|-------------------------------------------------------------------------------|--------------------|
| **gosec**       | SAST            | Credenciais hardcoded, SQL injection, permissões de arquivo, criptografia fraca | PR, push, semanal  |
| **staticcheck** | SAST            | Bugs, API deprecada, performance, correção de código                         | PR, push, semanal  |
| **govulncheck** | Dependency CVE  | CVEs em módulos Go (diretos e indiretos)                                      | PR, push, semanal  |
| **nancy**       | Dependency CVE  | Vulnerabilidades no Sonatype OSS Index                                        | PR, push, semanal  |

- **SAST** (Static Application Security Testing): análise estática do código-fonte.
- **Dependency CVE**: varredura de dependências conhecidas (go.mod / go.sum) contra bases de vulnerabilidades.

---

## 2. Como interpretar resultados no GitHub

1. Acesse o repositório no GitHub.
2. Abra a aba **Security**.
3. No menu lateral, clique em **Code scanning alerts** (ou **Vulnerability alerts** para dependências).
4. Os alertas do gosec aparecem em Code scanning (upload SARIF). Cada achado tem:
   - **Severity**: Critical, High, Medium, Low.
   - **Regra**: ex.: G201 (SQL injection), G304 (path traversal).

**Significado das severidades:**

- **Critical** / **High**: vulnerabilidades que devem ser corrigidas antes do merge.
- **Medium**: bloqueia por padrão; pode ser suprimido com justificativa documentada.
- **Low**: aviso; não bloqueia o merge, mas deve ser revisado.

Ao receber uma falha: corrigir o código ou a dependência, ou (apenas para Medium) suprimir com comentário obrigatório (veja seção 4).

---

## 3. Política de severidade — quando o PR é bloqueado

| Severidade | Efeito                                                                 |
|------------|------------------------------------------------------------------------|
| **CRITICAL** | Bloqueia merge obrigatoriamente.                                       |
| **HIGH**     | Bloqueia merge obrigatoriamente.                                      |
| **MEDIUM**   | Bloqueia por padrão; pode ser suprimido com justificativa no código.  |
| **LOW**      | Aviso apenas; não bloqueia merge.                                     |

O pipeline de segurança (workflow **Security**) falha se qualquer job (lint, SAST ou dependency-scan) falhar. Falha no pipeline impede merge até correção ou supressão justificada (apenas para MEDIUM).

---

## 4. Como suprimir um falso positivo

Toda supressão deve ter um comentário explicando o motivo. Supressões sem comentário são rejeitadas em code review.

**gosec** — use `#nosec` com o código da regra e motivo:

```go
// #nosec G201 -- falso positivo: input sanitizado em L42
query := fmt.Sprintf("SELECT * FROM users WHERE id = %s", sanitizedID)
```

**staticcheck** (via golangci-lint) — use `//nolint`:

```go
//nolint:staticcheck -- motivo breve (ex.: API legada, migração em andamento)
usoDeAPIDeprecada()
```

Regra: **todo comentário de supressão deve explicar o porquê**. Sem comentário = rejeição no code review.

---

## 5. Como reportar uma vulnerabilidade

- **Não abra uma issue pública** para vulnerabilidades de segurança.
- Envie um e-mail para: **security@example.com**  
  *(Substitua pelo e-mail real do projeto ou da organização.)*
- Prazo de resposta esperado: **48 horas úteis**.

---

## 6. Scan semanal automático

- O workflow **Security** roda automaticamente **toda segunda-feira às 8h UTC** no branch `main` (agendamento no GitHub Actions).
- Os resultados são enviados para a aba **Security** do repositório (Code scanning e dependências).
- O time pode ser notificado pelas notificações do GitHub (configuráveis em Settings → Notifications).

Para rodar os mesmos checks localmente antes do push:

```bash
./scripts/security-check.sh
```
