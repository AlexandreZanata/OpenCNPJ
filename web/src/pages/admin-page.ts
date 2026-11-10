/**
 * Administration for staff, over the admin API (`/admin/api/*`): accounts and their search
 * limits, plans and service-wide usage. The admin API answers only on the admin listener,
 * so this page appears when the application is opened there with a staff login.
 */

import { errorMessage, getJson, sendJson } from "../core/api.js";
import { h, type Child } from "../core/dom.js";
import { formatInteger } from "../core/format.js";
import { Badge, Button, Checkbox, DataTable, EmptyState, Field, Input, Meter, Select, Spinner, busy, confirmAction, copyText, openDrawer, openModal, toast, type Option, type Overlay } from "../ui/kit.js";
import type { Page } from "./page.js";

interface User {
  id: number;
  username: string;
  email: string;
  role: string;
  status: string;
  plan: string;
  created_at: number;
  last_seen_at: number | null;
  status_reason: string | null;
  notes: string;
  daily_limit: number | null;
  monthly_limit: number | null;
  per_minute: number | null;
  key_prefix?: string;
  usage_today?: number;
  usage_month?: number;
}

interface Plan {
  id: number;
  name: string;
  daily_limit: number | null;
  monthly_limit: number | null;
  per_minute: number | null;
  burst: number | null;
  max_page_size: number;
  export: boolean;
  max_export_rows: number | null;
  downloads: boolean;
  users?: number;
}

interface UsageReport {
  daily: { day: string; requests: number; users: number }[];
  by_kind: Record<string, number>;
  top_users_today: { id: number; username: string; count: number }[];
  totals: { users: number; active_users_today: number; requests_today: number; requests_month: number };
}

const ROLES: readonly Option[] = [
  ["user", "Usuário"],
  ["analyst", "Analista"],
  ["moderator", "Moderador"],
  ["admin", "Administrador"],
];
const STATUS_LABEL: Record<string, string> = { active: "Ativa", pending: "Pendente", suspended: "Suspensa", banned: "Bloqueada" };

function statusBadge(status: string): HTMLElement {
  return Badge(STATUS_LABEL[status] ?? status, status === "active" ? "good" : status === "pending" ? "neutral" : status === "suspended" ? "warn" : "bad");
}

function limit(value: number | null | undefined): string {
  return value === null || value === undefined ? "—" : formatInteger(value);
}

/** Empty means "use the plan's value" (`null`); anything else must be a whole number. */
function numberOrNull(input: HTMLInputElement): number | null {
  const text = input.value.trim();
  return text === "" ? null : Math.max(0, Math.trunc(Number(text)));
}

function numberInput(value: number | null | undefined, placeholder: string): HTMLInputElement {
  return Input({ type: "number", value: value === null || value === undefined ? "" : String(value), placeholder, inputmode: "numeric" });
}

function showKey(title: string, key: string): void {
  openModal(title, [
    h("p", { class: "muted" }, "Entregue esta chave ao usuário. Ela não será mostrada de novo."),
    h("code", { class: "secret" }, key),
    h("div", { class: "actions actions--end" }, Button({ label: "Copiar chave", icon: "copy", variant: "primary", onClick: () => void copyText(key, "Chave copiada.") })),
  ]);
}

export function AdminPage(): Page {
  const body = h("div", { class: "page__result" });
  const tabs = h("div", { class: "tabs", role: "tablist" });
  let plans: Plan[] = [];

  const fail = (error: unknown): void => toast(errorMessage(error) || "Não consegui concluir.", "error");

  // ----- users -----------------------------------------------------------------------------
  const usersTab = (): void => {
    const query = Input({ placeholder: "Buscar por usuário ou e-mail", class: "input--search" });
    const status = Select([["", "Todas as situações"], ["active", "Ativas"], ["pending", "Pendentes"], ["suspended", "Suspensas"], ["banned", "Bloqueadas"]]);
    const plan = Select([["", "Todos os planos"], ...plans.map((p): Option => [p.name, p.name])]);
    const list = h("div", null, Spinner());
    let timer = 0;

    const load = (): void => {
      const params = new URLSearchParams({ limit: "100", sort: "-usage_today" });
      if (query.value.trim()) params.set("q", query.value.trim());
      if (status.value) params.set("status", status.value);
      if (plan.value) params.set("plan", plan.value);
      getJson<{ total: number; users: User[] }>(`/admin/api/users?${params.toString()}`)
        .then((reply) => {
          list.replaceChildren(
            h("p", { class: "muted results__head" }, `${formatInteger(reply.total)} ${reply.total === 1 ? "conta" : "contas"}`),
            reply.users.length
              ? DataTable<User>(
                  [
                    { header: "Usuário", cell: (u) => [h("strong", null, u.username), h("span", { class: "cell-sub" }, u.email)], class: "cell-main" },
                    { header: "Papel", cell: (u) => ROLES.find(([value]) => value === u.role)?.[1] ?? u.role },
                    { header: "Plano", cell: (u) => u.plan },
                    { header: "Situação", cell: (u) => statusBadge(u.status) },
                    { header: "Hoje", cell: (u) => `${formatInteger(u.usage_today ?? 0)}${u.daily_limit !== null ? ` / ${formatInteger(u.daily_limit)}` : ""}`, class: "num" },
                    { header: "Mês", cell: (u) => formatInteger(u.usage_month ?? 0), class: "num" },
                    { header: "Último acesso", cell: (u) => (u.last_seen_at ? new Date(u.last_seen_at * 1000).toLocaleDateString("pt-BR") : "—") },
                  ],
                  reply.users,
                  (u) => openUser(u.id, load),
                )
              : EmptyState("users", "Nenhuma conta com esses filtros."),
          );
        })
        .catch(fail);
    };
    query.addEventListener("input", () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(load, 250);
    });
    status.addEventListener("change", load);
    plan.addEventListener("change", load);

    body.replaceChildren(
      h("div", { class: "toolbar" }, query, status, plan, Button({ label: "Nova conta", icon: "plus", variant: "primary", onClick: () => createUser(load) })),
      list,
    );
    load();
  };

  const createUser = (done: () => void): void => {
    const username = Input({ placeholder: "nome.sobrenome", required: true, maxlength: 32 });
    const email = Input({ type: "email", placeholder: "pessoa@empresa.com.br", required: true });
    const role = Select(ROLES, "user");
    const plan = Select(plans.map((p): Option => [p.name, p.name]), plans.find((p) => p.name === "free")?.name ?? plans[0]?.name ?? "");
    const mail = Checkbox("Enviar a chave por e-mail", true);
    const submit = Button({ label: "Criar conta", variant: "primary", type: "submit" });
    let modal: Overlay | null = null;
    modal = openModal(
      "Nova conta",
      h(
        "form",
        {
          class: "form",
          on: {
            submit: (event) => {
              event.preventDefault();
              void busy(submit, async () => {
                try {
                  const reply = await sendJson<{ user: User; key: string }>("POST", "/admin/api/users", {
                    username: username.value.trim().toLowerCase(),
                    email: email.value.trim(),
                    role: role.value,
                    plan: plan.value,
                    send_email: (mail.firstElementChild as HTMLInputElement).checked,
                  });
                  modal?.close();
                  done();
                  showKey(`Chave de ${reply.user.username}`, reply.key);
                } catch (error) {
                  fail(error);
                }
              });
            },
          },
        },
        Field({ label: "Nome de usuário", control: username }),
        Field({ label: "E-mail", control: email }),
        h("div", { class: "form__row" }, Field({ label: "Papel", control: role }), Field({ label: "Plano", control: plan })),
        mail,
        h("div", { class: "actions actions--end" }, submit),
      ),
    );
    username.focus();
  };

  const openUser = (id: number, done: () => void): void => {
    const host = h("div", { class: "record-host" }, Spinner());
    const drawer = openDrawer("Conta", host);
    const refresh = (): void => {
      getJson<{ user: User; usage_daily: { day: string; kind: string; count: number }[]; recent_searches: { ts: number; route: string; query: string; ms: number }[] }>(`/admin/api/users/${id}`)
        .then((detail) => {
          const u = detail.user;
          const today = new Date().toISOString().slice(0, 10);
          const month = today.slice(0, 7);
          const usedToday = detail.usage_daily.filter((row) => row.day === today).reduce((sum, row) => sum + row.count, 0);
          const usedMonth = detail.usage_daily.filter((row) => row.day.startsWith(month)).reduce((sum, row) => sum + row.count, 0);
          const current = plans.find((p) => p.name === u.plan);
          const plan = Select(plans.map((p): Option => [p.name, p.name]), u.plan);
          const role = Select(ROLES, u.role);
          const daily = numberInput(u.daily_limit, current?.daily_limit != null ? `plano: ${formatInteger(current.daily_limit)}` : "sem limite");
          const monthly = numberInput(u.monthly_limit, current?.monthly_limit != null ? `plano: ${formatInteger(current.monthly_limit)}` : "sem limite");
          const minute = numberInput(u.per_minute, current?.per_minute != null ? `plano: ${formatInteger(current.per_minute)}` : "sem limite");
          const notes = h("textarea", { class: "input input--area", rows: 2, placeholder: "Anotações internas" }, u.notes ?? "");
          const save = Button({ label: "Salvar alterações", variant: "primary", type: "submit" });
          const act = async (label: string, run: () => Promise<unknown>): Promise<void> => {
            try {
              await run();
              toast(label);
              refresh();
              done();
            } catch (error) {
              fail(error);
            }
          };
          const blocked = u.status === "suspended" || u.status === "banned";
          host.replaceChildren(
            h(
              "div",
              { class: "record" },
              h("header", { class: "record__head" }, h("div", null, h("h2", { class: "record__name" }, u.username), h("p", { class: "muted" }, u.email)), h("div", { class: "record__tags" }, statusBadge(u.status))),
              u.status_reason ? h("p", { class: "note" }, h("span", null, `Motivo: ${u.status_reason}`)) : null,
              h("div", { class: "meters" }, Meter("Hoje", usedToday, u.daily_limit ?? current?.daily_limit ?? null), Meter("Este mês", usedMonth, u.monthly_limit ?? current?.monthly_limit ?? null)),
              h(
                "form",
                {
                  class: "form",
                  on: {
                    submit: (event) => {
                      event.preventDefault();
                      void busy(save, () =>
                        act("Conta atualizada.", () =>
                          sendJson("PATCH", `/admin/api/users/${id}`, {
                            plan: plan.value,
                            role: role.value,
                            daily_limit: numberOrNull(daily),
                            monthly_limit: numberOrNull(monthly),
                            per_minute: numberOrNull(minute),
                            notes: notes.value,
                          }),
                        ),
                      );
                    },
                  },
                },
                h("h3", null, "Plano e limites de busca"),
                h("div", { class: "form__row" }, Field({ label: "Plano", control: plan }), Field({ label: "Papel", control: role })),
                h(
                  "div",
                  { class: "form__row form__row--3" },
                  Field({ label: "Por dia", control: daily }),
                  Field({ label: "Por mês", control: monthly }),
                  Field({ label: "Por minuto", control: minute }),
                ),
                h("p", { class: "field__hint" }, "Deixe em branco para usar o limite do plano."),
                Field({ label: "Notas", control: notes }),
                h("div", { class: "actions actions--end" }, save),
              ),
              h("h3", null, "Ações"),
              h(
                "div",
                { class: "actions actions--wrap" },
                blocked
                  ? Button({ label: "Reativar", icon: "check", onClick: () => void act("Conta reativada.", () => sendJson("POST", `/admin/api/users/${id}/reactivate`)) })
                  : Button({
                      label: "Suspender",
                      onClick: () => {
                        const reason = Input({ placeholder: "Ex.: uso acima do contratado", maxlength: 200 });
                        const confirm = Button({ label: "Suspender conta", variant: "danger", type: "submit" });
                        let modal: Overlay | null = null;
                        modal = openModal(
                          `Suspender ${u.username}?`,
                          h(
                            "form",
                            {
                              class: "form",
                              on: {
                                submit: (event) => {
                                  event.preventDefault();
                                  void busy(confirm, async () => {
                                    await act("Conta suspensa.", () => sendJson("POST", `/admin/api/users/${id}/suspend`, { reason: reason.value.trim() || "sem motivo informado" }));
                                    modal?.close();
                                  });
                                },
                              },
                            },
                            h("p", { class: "muted" }, "A conta deixa de consultar até ser reativada. O motivo fica registrado para a equipe."),
                            Field({ label: "Motivo", control: reason }),
                            h("div", { class: "actions actions--end" }, confirm),
                          ),
                        );
                        reason.focus();
                      },
                    }),
                Button({ label: "Zerar uso de hoje", onClick: () => void act("Uso de hoje zerado.", () => sendJson("POST", `/admin/api/users/${id}/reset-usage`, { scope: "day" })) }),
                Button({ label: "Encerrar sessões", onClick: () => void act("Sessões encerradas.", () => sendJson("POST", `/admin/api/users/${id}/revoke-sessions`)) }),
                Button({
                  label: "Nova chave",
                  icon: "key",
                  onClick: () =>
                    void (async () => {
                      if (!(await confirmAction("Gerar nova chave?", `A chave atual de ${u.username} deixa de funcionar agora.`, "Gerar nova chave", true))) return;
                      try {
                        const reply = await sendJson<{ key: string }>("POST", `/admin/api/users/${id}/rotate-key`, { send_email: false });
                        showKey(`Nova chave de ${u.username}`, reply.key);
                        refresh();
                      } catch (error) {
                        fail(error);
                      }
                    })(),
                }),
                Button({
                  label: "Excluir conta",
                  variant: "danger",
                  onClick: () =>
                    void (async () => {
                      if (!(await confirmAction("Excluir esta conta?", `A conta ${u.username}, sua chave e seu histórico de uso são apagados. Isso não pode ser desfeito.`, "Excluir conta", true))) return;
                      try {
                        await sendJson("DELETE", `/admin/api/users/${id}`);
                        toast("Conta excluída.");
                        drawer.close();
                        done();
                      } catch (error) {
                        fail(error);
                      }
                    })(),
                }),
              ),
              detail.recent_searches.length
                ? [
                    h("h3", null, "Consultas recentes"),
                    h(
                      "ul",
                      { class: "log" },
                      detail.recent_searches.slice(0, 12).map((row) =>
                        h("li", null, h("span", { class: "muted" }, new Date(row.ts * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" })), h("span", null, safeDecode(row.query) || row.route)),
                      ),
                    ),
                  ]
                : null,
            ),
          );
        })
        .catch((error: unknown) => host.replaceChildren(EmptyState("info", errorMessage(error) || "Não consegui abrir a conta.")));
    };
    refresh();
  };

  // ----- plans -----------------------------------------------------------------------------
  const plansTab = (): void => {
    const render = (): void => {
      body.replaceChildren(
        h("div", { class: "toolbar toolbar--end" }, Button({ label: "Novo plano", icon: "plus", variant: "primary", onClick: () => editPlan(null) })),
        DataTable<Plan>(
          [
            { header: "Plano", cell: (p) => h("strong", null, p.name), class: "cell-main" },
            { header: "Por dia", cell: (p) => limit(p.daily_limit), class: "num" },
            { header: "Por mês", cell: (p) => limit(p.monthly_limit), class: "num" },
            { header: "Por minuto", cell: (p) => limit(p.per_minute), class: "num" },
            { header: "Exportação", cell: (p) => (p.export ? (p.max_export_rows ? `até ${formatInteger(p.max_export_rows)} linhas` : "sem limite") : "não") },
            { header: "Contas", cell: (p) => formatInteger(p.users ?? 0), class: "num" },
          ],
          plans,
          (p) => editPlan(p),
        ),
        h("p", { class: "muted" }, "“—” significa sem limite. Uma conta pode ter limites próprios, que valem no lugar dos do plano."),
      );
    };
    const editPlan = (plan: Plan | null): void => {
      const name = Input({ value: plan?.name ?? "", placeholder: "equipe-comercial", required: true });
      const daily = numberInput(plan?.daily_limit, "sem limite");
      const monthly = numberInput(plan?.monthly_limit, "sem limite");
      const minute = numberInput(plan?.per_minute, "sem limite");
      const pageSize = numberInput(plan?.max_page_size ?? 100, "100");
      const exportRows = numberInput(plan?.max_export_rows, "sem limite");
      const exports = Checkbox("Permite exportar arquivos", plan?.export ?? false);
      const downloads = Checkbox("Permite baixar a base completa", plan?.downloads ?? false);
      const save = Button({ label: plan ? "Salvar plano" : "Criar plano", variant: "primary", type: "submit" });
      let modal: Overlay | null = null;
      modal = openModal(
        plan ? `Plano ${plan.name}` : "Novo plano",
        h(
          "form",
          {
            class: "form",
            on: {
              submit: (event) => {
                event.preventDefault();
                void busy(save, async () => {
                  const payload = {
                    name: name.value.trim(),
                    daily_limit: numberOrNull(daily),
                    monthly_limit: numberOrNull(monthly),
                    per_minute: numberOrNull(minute),
                    burst: plan?.burst ?? null,
                    max_page_size: numberOrNull(pageSize) ?? 100,
                    export: (exports.firstElementChild as HTMLInputElement).checked,
                    max_export_rows: numberOrNull(exportRows),
                    downloads: (downloads.firstElementChild as HTMLInputElement).checked,
                  };
                  try {
                    await (plan ? sendJson("PATCH", `/admin/api/plans/${plan.id}`, payload) : sendJson("POST", "/admin/api/plans", payload));
                    plans = await getJson<Plan[]>("/admin/api/plans");
                    modal?.close();
                    toast("Plano salvo.");
                    render();
                  } catch (error) {
                    fail(error);
                  }
                });
              },
            },
          },
          Field({ label: "Nome", control: name }),
          h("div", { class: "form__row form__row--3" }, Field({ label: "Buscas por dia", control: daily }), Field({ label: "Buscas por mês", control: monthly }), Field({ label: "Por minuto", control: minute })),
          h("div", { class: "form__row" }, Field({ label: "Resultados por página", control: pageSize }), Field({ label: "Linhas por arquivo", control: exportRows })),
          exports,
          downloads,
          h("div", { class: "actions actions--end" }, save),
        ),
      );
    };
    render();
  };

  // ----- usage -----------------------------------------------------------------------------
  const usageTab = (): void => {
    body.replaceChildren(Spinner());
    getJson<UsageReport>("/admin/api/usage?days=30")
      .then((report) => {
        const max = Math.max(1, ...report.daily.map((d) => d.requests));
        const tile = (label: string, value: number): Child => h("div", { class: "kpi" }, h("span", { class: "kpi__value" }, formatInteger(value)), h("span", { class: "kpi__label" }, label));
        body.replaceChildren(
          h("div", { class: "kpis" }, tile("Contas", report.totals.users), tile("Ativas hoje", report.totals.active_users_today), tile("Consultas hoje", report.totals.requests_today), tile("Consultas no mês", report.totals.requests_month)),
          h(
            "section",
            { class: "card" },
            h("h2", { class: "card__title" }, "Consultas por dia (30 dias)"),
            h(
              "div",
              { class: "bars", role: "img", "aria-label": "Consultas por dia" },
              report.daily.map((d) => h("span", { class: "bars__bar", title: `${d.day}: ${formatInteger(d.requests)} consultas, ${formatInteger(d.users)} contas`, style: `height:${Math.max(d.requests ? 6 : 2, (d.requests / max) * 100).toFixed(1)}%` })),
            ),
          ),
          h(
            "section",
            { class: "card" },
            h("h2", { class: "card__title" }, "Quem mais consultou hoje"),
            report.top_users_today.length
              ? DataTable<{ id: number; username: string; count: number }>(
                  [
                    { header: "Usuário", cell: (row) => h("strong", null, row.username), class: "cell-main" },
                    { header: "Consultas", cell: (row) => formatInteger(row.count), class: "num" },
                  ],
                  report.top_users_today,
                  (row) => openUser(row.id, usageTab),
                )
              : h("p", { class: "muted" }, "Nenhuma consulta hoje."),
          ),
        );
      })
      .catch((error: unknown) => body.replaceChildren(EmptyState("info", errorMessage(error) || "Não consegui carregar o uso.")));
  };

  const TABS: readonly (readonly [string, () => void])[] = [
    ["Contas", usersTab],
    ["Planos", plansTab],
    ["Uso", usageTab],
  ];
  const select = (index: number): void => {
    tabs.querySelectorAll("button").forEach((tab, i) => tab.setAttribute("aria-selected", String(i === index)));
    TABS[index]?.[1]();
  };
  TABS.forEach(([label], i) => tabs.append(h("button", { class: "tab", type: "button", role: "tab", on: { click: () => select(i) } }, label)));

  body.replaceChildren(Spinner());
  getJson<Plan[]>("/admin/api/plans")
    .then((loaded) => {
      plans = loaded;
      select(0);
    })
    .catch((error: unknown) => body.replaceChildren(EmptyState("shield", errorMessage(error) || "A administração não está disponível neste endereço.", "Abra o sistema pelo endereço administrativo e entre com uma conta da equipe.")));

  return { el: h("div", { class: "page page--wide" }, h("h1", { class: "page__title" }, "Administração"), tabs, body) };
}

function safeDecode(query: string): string {
  try {
    return decodeURIComponent(query.replace(/\+/g, " "));
  } catch {
    return query;
  }
}
