/** The user's own account: plan and limits, usage, login key and recent searches. */

import { errorMessage, getJson, loadAccount, login, logout, sendJson } from "../core/api.js";
import { h, type Child } from "../core/dom.js";
import { formatInteger } from "../core/format.js";
import { session, type Account } from "../core/session.js";
import { Badge, Button, DataTable, EmptyState, Meter, Spinner, busy, confirmAction, copyText, openModal, toast } from "../ui/kit.js";
import type { Page } from "./page.js";

interface UsageDay {
  day: string;
  kind: string;
  count: number;
}

interface SearchRow {
  ts: number;
  route: string;
  query: string;
  status: number;
  ms: number;
}

interface Activity {
  usage_daily: UsageDay[];
  recent_searches: SearchRow[];
}

const KINDS: Record<string, string> = {
  assistant: "Assistente",
  search: "Busca",
  lookup: "CNPJ",
  stats: "Estatísticas",
  export: "Exportação",
  cpf: "CPF",
  download: "Download",
};

function card(title: string, ...body: Child[]): HTMLElement {
  return h("section", { class: "card" }, h("h2", { class: "card__title" }, title), body);
}

/** Requests per day over the last thirty days, as plain bars. */
function usageBars(rows: readonly UsageDay[]): HTMLElement {
  const byDay = new Map<string, number>();
  for (const row of rows) byDay.set(row.day, (byDay.get(row.day) ?? 0) + row.count);
  const days: string[] = [];
  const today = new Date();
  for (let back = 29; back >= 0; back--) {
    const date = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate() - back));
    days.push(date.toISOString().slice(0, 10));
  }
  const max = Math.max(1, ...days.map((day) => byDay.get(day) ?? 0));
  return h(
    "div",
    { class: "bars", role: "img", "aria-label": "Consultas por dia nos últimos 30 dias" },
    days.map((day) => {
      const count = byDay.get(day) ?? 0;
      return h("span", { class: "bars__bar", title: `${day.slice(8)}/${day.slice(5, 7)}: ${formatInteger(count)}`, style: `height:${Math.max(count ? 6 : 2, (count / max) * 100).toFixed(1)}%` });
    }),
  );
}

export function AccountPage(): Page {
  const root = h("div", { class: "page page--narrow" }, Spinner("Carregando a conta"));

  const render = (account: Account, activity: Activity | null): void => {
    const { plan, limits, usage, user } = account;
    const origin = window.location.origin;
    const byKind = new Map<string, number>();
    for (const row of activity?.usage_daily ?? []) byKind.set(row.kind, (byKind.get(row.kind) ?? 0) + row.count);

    const rotate = Button({
      label: "Gerar nova chave",
      icon: "refresh",
      onClick: () =>
        void (async () => {
          const sure = await confirmAction(
            "Gerar nova chave?",
            "A chave atual e todas as sessões abertas deixam de funcionar agora. Integrações que usam a chave antiga precisam ser atualizadas.",
            "Gerar nova chave",
            true,
          );
          if (!sure) return;
          await busy(rotate, async () => {
            try {
              const reply = await sendJson<{ key: string }>("POST", "/api/v1/auth/key/rotate");
              await login(reply.key);
              openModal("Sua nova chave", [
                h("p", { class: "muted" }, "Guarde-a agora: ela não será mostrada de novo. Uma cópia foi enviada para o seu e-mail."),
                h("code", { class: "secret" }, reply.key),
                h("div", { class: "actions actions--end" }, Button({ label: "Copiar chave", icon: "copy", variant: "primary", onClick: () => void copyText(reply.key, "Chave copiada.") })),
              ]);
            } catch (error) {
              toast(errorMessage(error), "error");
            }
          });
        })(),
    });

    root.replaceChildren(
      h(
        "header",
        { class: "page__head" },
        h("div", null, h("h1", { class: "page__title" }, user.username), h("p", { class: "muted" }, user.email)),
        Button({ label: "Sair", icon: "log-out", onClick: () => void logout() }),
      ),
      card(
        "Plano e uso",
        plan.name === "free"
          ? h(
              "p",
              { class: "note" },
              "Para ter mais consultas e exportar resultados, solicite um plano superior. ",
              session().platform?.upgrade_url
                ? h("a", { href: session().platform?.upgrade_url ?? undefined }, "Solicitar upgrade")
                : "Fale com o administrador para ativar o plano.",
            )
          : null,
        h("p", { class: "plan" }, Badge(plan.name, "good"), limits.per_minute ? h("span", { class: "muted" }, `até ${formatInteger(limits.per_minute)} consultas por minuto`) : null),
        h("div", { class: "meters" }, Meter("Hoje", usage.today, limits.daily_limit), Meter("Este mês", usage.month, limits.monthly_limit)),
        h(
          "ul",
          { class: "features" },
          h("li", null, plan.export ? `Exportação de arquivos${plan.max_export_rows ? `, até ${formatInteger(plan.max_export_rows)} linhas por arquivo` : ""}` : "Sem exportação de arquivos"),
          h("li", null, `Até ${formatInteger(plan.max_page_size)} resultados por página na API`),
          h("li", null, "Cada pergunta ao assistente, busca ou consulta de CNPJ conta como uma consulta. Os limites reiniciam à meia-noite (UTC) e no dia 1º."),
        ),
      ),
      card(
        "Últimos 30 dias",
        activity
          ? [
              usageBars(activity.usage_daily),
              byKind.size
                ? h("p", { class: "kinds" }, [...byKind.entries()].sort((a, b) => b[1] - a[1]).map(([kind, count]) => h("span", { class: "tag" }, `${KINDS[kind] ?? kind}: ${formatInteger(count)}`)))
                : h("p", { class: "muted" }, "Nenhuma consulta registrada ainda."),
            ]
          : h("p", { class: "muted" }, "Não consegui carregar o histórico de uso."),
      ),
      card(
        "Chave de API",
        h("p", { class: "muted" }, "Uma única chave entra no site e autentica a API. Ela é mostrada só quando é criada."),
        h("div", { class: "keyline" }, h("code", null, `ocnpj_${user.key_prefix ?? "…"}_••••••••••••`), rotate),
        h("details", { class: "provenance" }, h("summary", null, "Como usar a API"), [
          h("p", { class: "muted" }, "Envie a chave no cabeçalho Authorization. Os cabeçalhos X-Quota-* de cada resposta mostram quanto resta do plano."),
          h(
            "pre",
            { class: "code" },
            [
              `# Pergunta em português`,
              `curl -X POST ${origin}/api/v1/assistant/ask \\`,
              `  -H "Authorization: Bearer $OPENCNPJ_KEY" -H "Content-Type: application/json" \\`,
              `  -d '{"question": "dentistas em Cuiabá/MT com telefone"}'`,
              ``,
              `# Busca por filtros`,
              `curl -H "Authorization: Bearer $OPENCNPJ_KEY" \\`,
              `  "${origin}/api/v1/search?uf=MT&cnae=4771701&situacao=02&tem_telefone=true"`,
              ``,
              `# Ficha de um CNPJ`,
              `curl -H "Authorization: Bearer $OPENCNPJ_KEY" ${origin}/api/v1/cnpj/33000167000101`,
            ].join("\n"),
          ),
        ]),
      ),
      card(
        "Consultas recentes",
        activity?.recent_searches.length
          ? DataTable<SearchRow>(
              [
                { header: "Quando", cell: (row) => new Date(row.ts * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" }) },
                { header: "Tipo", cell: (row) => (row.route.includes("/assistant/") ? "Assistente" : row.route.includes("/cnpj") ? "CNPJ" : row.route.includes("/export") ? "Exportação" : "Busca") },
                { header: "Consulta", cell: (row) => decodeQuery(row.query), class: "cell-wrap" },
                { header: "ms", cell: (row) => row.ms.toFixed(0), class: "num" },
              ],
              activity.recent_searches.slice(0, 20),
            )
          : h("p", { class: "muted" }, "As consultas aparecem aqui quando o registro de buscas está ligado no servidor."),
      ),
    );
  };

  const account = session().account;
  const start = account ? Promise.resolve(account) : loadAccount();
  void start.then(async (loaded) => {
    if (!loaded) {
      root.replaceChildren(EmptyState("user", "Não consegui carregar a sua conta.", "Entre novamente para continuar."));
      return;
    }
    const fresh = (await loadAccount()) ?? loaded;
    const activity = await getJson<Activity>("/api/v1/auth/usage").catch(() => null);
    render(fresh, activity);
  });

  return { el: root };
}

function decodeQuery(query: string): string {
  try {
    return decodeURIComponent(query.replace(/\+/g, " ")).replace(/&/g, " · ");
  } catch {
    return query;
  }
}
