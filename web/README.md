# OpenCNPJ Web

Frontend da plataforma OpenCNPJ para inteligência empresarial e consulta em linguagem natural com dados oficiais do CNPJ (Receita Federal).

Construído de forma 100% desacoplada, utilizando exclusivamente **TypeScript 7.0.2** compilado para módulos JavaScript ES2022 nativos, **HTML5** e **CSS3** puro, com **zero dependências em runtime** (sem React, Vue, Angular, Tailwind, Webpack, etc.).

## Estrutura do Projeto

```
web/
├── package.json          # Metadados e scripts de build/dev (0 dependências de runtime)
├── tsconfig.json         # Configuração TypeScript 7 (ES2022, target nativo)
├── public/               # Assets estáticos (HTML, CSS modular, fontes locais)
│   ├── index.html        # Shell HTML semântico com preloads de fontes
│   ├── fonts/            # Fontes self-hosted (Inter e Source Serif 4 em WOFF2)
│   └── styles/           # CSS modular baseado em design tokens
│       ├── tokens.css    # Paleta de cores, tipografia, elevações e espaçamentos
│       ├── base.css      # Reset semântico e estilos globais
│       ├── layout.css    # Grid responsivo (sidebar + workspace)
│       └── components/   # Estilos isolados por componente reutilizável
├── src/                  # Código TypeScript desacoplado
│   ├── config.ts         # Parâmetros e URL da API
│   ├── main.ts           # Ponto de entrada e bootstrap
│   ├── core/             # Primitivas fundamentais (DOM, store, router, formatters)
│   ├── domain/           # Modelos de domínio e tipos da plataforma
│   ├── services/         # Camada de serviços e integração (mock + contrato API)
│   ├── state/            # Controllers e gestão de estado reativo
│   ├── ui/               # Componentes visuais atômicos e reutilizáveis
│   ├── features/         # Módulos de negócio (layout, query com IA, insights, tabela)
│   └── pages/            # Páginas da aplicação e placeholders de navegação
├── scripts/
│   ├── build.sh          # Compila TS para dist/ e copia assets
│   ├── serve.sh          # Inicia servidor estático de desenvolvimento
│   └── tsc.sh            # Invoca o compilador TypeScript 7
└── dist/                 # Artefatos finais compilados (HTML/CSS/JS nativo)
```

## Como Rodar Localmente

### Pré-requisitos
Apenas Python 3 (para servir os arquivos estáticos) ou qualquer servidor HTTP estático.
O frontend compilado em `dist/` não requer instalação de pacotes npm em runtime.

### 1. Iniciar o servidor
```bash
cd web
sh scripts/serve.sh
# ou: python3 -m http.server 5173 --bind 127.0.0.1 --directory dist
```
Acesse no navegador:
👉 [http://127.0.0.1:5173/](http://127.0.0.1:5173/)

### 2. Recompilar alterações de TypeScript
```bash
cd web
sh scripts/build.sh
```

## Design e Fidelidade Visual
O layout implementa exatamente a interface de inteligência empresarial:
- **Sidebar**: Navegação completa da plataforma com rotas via hash (`#/consulta`, `#/explorar`, etc.) e badge da Receita Federal.
- **Header & Hero**: Arte decorativa com gradientes em vidro, sininho com indicador de notificação e menu do plano corporativo.
- **Consulta com IA**: Campo em linguagem natural, botões de atalho/exemplos e banner de filtros verificáveis derivados pela IA.
- **KPIs & Métricas**: Cartões com números formatados (ex: R$ 318,6 milhões), tendências percentuais e contadores.
- **Gráficos em CSS/SVG**: Gráfico de aberturas mensais com eixos e gráfico donut de distribuição por porte (ME, EPP, Demais).
- **Tabela de Empresas**: Tabela com ordenação por colunas, menu para exibir/ocultar colunas, exportação (CSV / NDJSON) e paginação completa.
- **Design Responsivo**: Adaptável para desktop (1536px), notebooks e dispositivos móveis.
