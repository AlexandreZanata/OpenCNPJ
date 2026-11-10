/** One assistant turn, rendered from the typed blocks of `POST /api/v1/assistant/ask`. */
import { errorMessage, request, saveAs } from "../../core/api.js";
import { h } from "../../core/dom.js";
import { formatInteger } from "../../core/format.js";
import { session } from "../../core/session.js";
import { Icon } from "../../ui/icon.js";
import { Button, Chip, busy } from "../../ui/kit.js";
import { openCompany } from "../company/company-view.js";
import { BarList, LineChart, formatValue } from "./charts.js";
const COMPANY_KEYS = { cnpj: "CNPJ", razao_social: "Razão social", situacao: "Situação cadastral" };
/** Emphasizes the measured numbers of a sentence ("1.234", "12,5%"), not the years. */
function lead(text) {
    const parts = [];
    let last = 0;
    for (const match of text.matchAll(/\d+(?:\.\d{3})*(?:,\d+)?%?/g)) {
        const at = match.index ?? 0;
        if (/^(19|20)\d{2}$/.test(match[0]))
            continue;
        parts.push(text.slice(last, at), h("strong", null, match[0]));
        last = at + match[0].length;
    }
    parts.push(text.slice(last));
    return h("p", { class: "answer__lead" }, parts);
}
function table(columns, rows, collapse = Infinity) {
    const numeric = columns.map((_, i) => rows.length > 0 && rows.every((row) => /^-?[\d.]+(,\d+)?%?$|^sem dado$/.test(row[i] ?? "")));
    const cnpjColumn = columns[0] === "CNPJ";
    const body = h("tbody", null, rows.map((row, r) => h("tr", { hidden: r >= collapse }, row.map((cell, i) => i === 0
        ? h("th", { scope: "row" }, cnpjColumn && cell
            ? h("button", { class: "link mono", type: "button", title: "Abrir a ficha da empresa", on: { click: () => openCompany(cell) } }, cell)
            : cell)
        : h("td", { class: numeric[i] ? "num" : undefined }, cell)))));
    const more = rows.length > collapse
        ? h("button", {
            class: "link",
            type: "button",
            on: {
                click: (event) => {
                    body.querySelectorAll("tr[hidden]").forEach((tr) => (tr.hidden = false));
                    event.currentTarget.remove();
                },
            },
        }, `Mostrar as ${formatInteger(rows.length)} linhas`)
        : null;
    return h("div", { class: "answer__table" }, h("div", { class: "table-wrap chat-table-wrap" }, h("table", { class: "table chat-table" }, h("thead", null, h("tr", null, columns.map((column, i) => h("th", { scope: "col", class: numeric[i] ? "num" : undefined }, column)))), body)), more);
}
function records(title, columns, rows, total) {
    return h("section", { class: "answer__section" }, h("header", { class: "answer__section-head" }, h("h3", null, title ?? "Estabelecimentos"), h("span", { class: "muted" }, total > rows.length ? `${formatInteger(rows.length)} de ${formatInteger(total)}` : formatInteger(total))), table(columns, rows, 10));
}
function exportCard(offer, question) {
    const status = h("span", { class: "muted", role: "status" });
    const plan = session().account?.plan;
    const allowed = !plan || plan.export;
    const button = Button({
        label: `Baixar ${offer.format.toUpperCase()}`,
        icon: "download",
        variant: "primary",
        disabled: offer.rows === 0 || !allowed,
        onClick: () => void busy(button, async () => {
            status.textContent = "Gerando o arquivo…";
            try {
                const response = await request("/api/v1/assistant/export", { method: "POST", body: { request: offer.request, export: offer.export, question } });
                await saveAs(response, offer.filename);
                status.textContent = `Arquivo baixado: ${formatInteger(Number(response.headers.get("x-opencnpj-rows") ?? offer.rows))} linhas.`;
            }
            catch (error) {
                status.textContent = errorMessage(error) || "Não consegui gerar o arquivo.";
            }
        }),
    });
    if (!allowed)
        status.textContent = `O plano ${plan?.name ?? ""} não inclui exportação de arquivos.`;
    return h("section", { class: "file", "aria-label": "Arquivo da resposta" }, h("span", { class: "file__icon" }, Icon({ name: "file", size: 20 })), h("div", { class: "file__body" }, h("strong", null, offer.filename), h("span", { class: "muted" }, `${offer.label} · ${formatInteger(offer.rows)} ${offer.rows === 1 ? "linha" : "linhas"} · ${offer.columns.join(", ")}`), offer.notes.map((note) => h("span", { class: "muted" }, note)), status), button);
}
function block(item, actions, first, metric = "Valor") {
    switch (item.type) {
        case "text":
            return first ? lead(item.text) : h("p", null, item.text);
        case "kpi":
            return h("div", { class: "kpi" }, h("span", { class: "kpi__value" }, item.value), h("span", { class: "kpi__label" }, item.label));
        case "comparison":
            return h("div", { class: "answer__section" }, h("div", { class: "kpis" }, item.members.map((m) => h("div", { class: "kpi" }, h("span", { class: "kpi__value" }, m.value), h("span", { class: "kpi__label" }, m.label)))), h("p", { class: "muted" }, `Diferença: ${item.difference_abs}${item.difference_pct ? ` (${item.difference_pct})` : ""}`));
        case "chart":
            return item.kind === "line" ? LineChart(metric, [{ name: metric, points: item.series }]) : BarList(metric, item.series);
        case "table":
            return table(item.columns, item.rows, 12);
        case "records":
            return records(null, item.columns, item.rows, item.total);
        case "company": {
            const cnpj = item.fields.find(([key]) => key === "cnpj")?.[1];
            return h("section", { class: "answer__section" }, h("dl", { class: "facts facts--card" }, item.fields.map(([key, value]) => [h("dt", null, COMPANY_KEYS[key] ?? key), h("dd", { class: key === "cnpj" ? "mono" : undefined }, value)])), cnpj ? Button({ label: "Abrir a ficha completa", icon: "building", size: "sm", onClick: () => openCompany(cnpj) }) : null);
        }
        case "notice":
            return h("p", { class: "note" }, Icon({ name: "info", size: 15 }), h("span", null, item.text));
        case "clarify":
            return h("div", { class: "answer__ask" }, h("p", { class: "answer__lead" }, item.question), item.options.length ? h("div", { class: "chips" }, item.options.map((o) => Chip(o.label, () => actions.onFollowUp(o.reply)))) : null);
        case "reject":
            return h("p", { class: "answer__lead answer__lead--muted" }, item.reason);
        case "methodology":
        case "sources":
        case "caveats":
            return null;
    }
}
function provenance(blocks) {
    const items = [];
    for (const item of blocks) {
        if (item.type === "methodology")
            items.push(h("h4", null, "Filtros e método"), h("ul", null, item.items.map((text) => h("li", null, text))));
        if (item.type === "caveats")
            items.push(h("h4", null, "Cuidados"), h("ul", null, item.items.map((c) => h("li", null, c.text))));
        if (item.type === "sources")
            items.push(h("h4", null, "Fonte"), h("ul", null, item.items.map((src) => h("li", null, `${src.name}, competência ${src.competence}`, src.snapshot_sha256 ? h("code", { title: src.snapshot_sha256 }, ` · snapshot ${src.snapshot_sha256.slice(0, 12)}…`) : null))));
    }
    return items.length ? h("details", { class: "provenance" }, h("summary", null, "Como cheguei a esse resultado"), items) : null;
}
function extra(item) {
    if (item.type === "series")
        return LineChart(item.title, item.series);
    return records(item.title, item.columns, item.rows, item.total);
}
/** The answer of one turn. `question` is sent back with a file request, for its "Sobre" sheet. */
export function AnswerView(reply, question, actions) {
    const { answer } = reply;
    const blocks = answer.blocks;
    const kpis = blocks.filter((b) => b.type === "kpi");
    const charts = blocks.filter((b) => b.type === "chart");
    const rest = blocks.filter((b) => b.type !== "kpi" && b.type !== "chart");
    const firstText = blocks.findIndex((b) => b.type === "text");
    const total = reply.timing_us["total"] ?? 0;
    // What was understood, as tags: "estabelecimentos · padaria · Sinop (MT) · RFB 2026-10".
    const understood = answer.understood.split(" · ").filter(Boolean);
    const subject = understood.filter((part) => !part.startsWith("RFB ")).join(" · ");
    const metric = subject ? subject.charAt(0).toUpperCase() + subject.slice(1) : "Resultado";
    return h("div", { class: "answer" }, rest.filter((b) => b.type === "text" || b.type === "clarify" || b.type === "reject").map((b) => block(b, actions, blocks.indexOf(b) === firstText)), rest.filter((b) => b.type === "notice").map((b) => block(b, actions, false)), kpis.length > 1 ? h("div", { class: "kpis" }, kpis.map((b) => block(b, actions, false))) : null, reply.export ? exportCard(reply.export, question) : null, charts.map((b) => block(b, actions, false, metric)), reply.extras.filter((e) => e.type === "series").map(extra), rest.filter((b) => ["comparison", "table", "records", "company"].includes(b.type)).map((b) => block(b, actions, false)), reply.extras.filter((e) => e.type === "records").map(extra), answer.suggestions.length ? h("div", { class: "chips" }, answer.suggestions.map((text) => Chip(text, () => actions.onFollowUp(text)))) : null, provenance(blocks), understood.length || total
        ? h("p", { class: "answer__meta" }, understood.filter((part) => !part.startsWith("RFB ")).map((part) => h("span", { class: "tag" }, part)), (answer.filters ?? []).map((part) => h("span", { class: "tag tag--filter" }, part)), understood.filter((part) => part.startsWith("RFB ")).map((part) => h("span", { class: "tag" }, part)), total ? h("span", null, `${formatValue(total / 1000)} ms`) : null)
        : null);
}
//# sourceMappingURL=answer-view.js.map