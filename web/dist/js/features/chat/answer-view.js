import { h } from "../../core/dom.js";
import { formatInteger } from "../../core/format.js";
import { Icon } from "../../ui/icon.js";
import { download } from "../../services/local-assistant.js";
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
    return h("p", { class: "chat-lead" }, parts);
}
function table(columns, rows, options = {}) {
    const numeric = columns.map((_, i) => rows.length > 0 && rows.every((row) => /^-?[\d.]+(,\d+)?%?$|^sem dado$/.test(row[i] ?? "")));
    const collapse = options.collapse ?? Infinity;
    const body = h("tbody", null, rows.map((row, r) => h("tr", { hidden: r >= collapse }, row.map((cell, i) => i === 0 ? h("th", { scope: "row", class: options.cnpj ? "mono" : undefined }, cell) : h("td", { class: numeric[i] ? "num" : undefined }, cell)))));
    const more = rows.length > collapse
        ? h("button", {
            class: "chat-link",
            type: "button",
            on: {
                click: (event) => {
                    body.querySelectorAll("tr[hidden]").forEach((tr) => (tr.hidden = false));
                    event.currentTarget.remove();
                },
            },
        }, `Mostrar todas as ${formatInteger(rows.length)} linhas`)
        : null;
    return h("div", { class: "chat-table-block" }, h("div", { class: "chat-table-wrap" }, h("table", { class: "chat-table" }, h("thead", null, h("tr", null, columns.map((column, i) => h("th", { scope: "col", class: numeric[i] ? "num" : undefined }, column)))), body)), more);
}
function records(title, columns, rows, total) {
    return h("section", { class: "chat-records" }, h("header", { class: "chat-section-head" }, h("h3", null, title ?? "Estabelecimentos"), h("span", { class: "chat-muted" }, `${formatInteger(rows.length)} de ${formatInteger(total)}`)), table(columns, rows, { cnpj: columns[0] === "CNPJ", collapse: 10 }));
}
function exportCard(offer, question) {
    const status = h("span", { class: "chat-muted", role: "status" });
    const label = `Baixar ${offer.format.toUpperCase()}`;
    const button = h("button", {
        class: "btn btn--primary chat-export__button",
        type: "button",
        disabled: offer.rows === 0,
        on: {
            click: async () => {
                button.disabled = true;
                status.textContent = "Gerando o arquivo…";
                try {
                    const rows = await download(offer, question);
                    status.textContent = `Arquivo baixado: ${formatInteger(rows)} linhas.`;
                }
                catch (error) {
                    status.textContent = error instanceof Error ? error.message : "Não consegui gerar o arquivo.";
                }
                finally {
                    button.disabled = false;
                }
            },
        },
    }, Icon({ name: "download", size: 18 }), label);
    return h("section", { class: "chat-export", "aria-label": "Arquivo da resposta" }, h("span", { class: "chat-export__icon" }, Icon({ name: "file", size: 24 })), h("div", { class: "chat-export__body" }, h("strong", null, offer.filename), h("span", { class: "chat-muted" }, `${offer.label} · ${formatInteger(offer.rows)} ${offer.rows === 1 ? "linha" : "linhas"} · ${offer.columns.length} colunas: ${offer.columns.join(", ")}`), offer.notes.map((note) => h("span", { class: "chat-note" }, note)), status), button);
}
function block(item, actions, first, metric = "Valor") {
    switch (item.type) {
        case "text":
            return first ? lead(item.text) : h("p", null, item.text);
        case "kpi":
            return h("div", { class: "chat-kpi" }, h("span", { class: "chat-kpi__value" }, item.value), h("span", { class: "chat-kpi__label" }, item.label));
        case "comparison":
            return h("div", { class: "chat-compare" }, item.members.map((m) => h("div", { class: "chat-kpi" }, h("span", { class: "chat-kpi__value" }, m.value), h("span", { class: "chat-kpi__label" }, m.label))), h("p", { class: "chat-muted" }, `Diferença: ${item.difference_abs}${item.difference_pct ? ` (${item.difference_pct})` : ""}`));
        case "chart":
            return item.kind === "line" ? LineChart(metric, [{ name: metric, points: item.series }]) : BarList(metric, item.series);
        case "table":
            return table(item.columns, item.rows, { collapse: 12 });
        case "records":
            return records(null, item.columns, item.rows, item.total);
        case "company":
            return h("dl", { class: "chat-company" }, item.fields.map(([key, value]) => [h("dt", null, COMPANY_KEYS[key] ?? key), h("dd", { class: key === "cnpj" ? "mono" : undefined }, value)]));
        case "notice":
            return h("p", { class: "chat-notice" }, Icon({ name: "sparkle", size: 16 }), item.text);
        case "clarify":
            return h("div", { class: "chat-clarify" }, h("p", null, item.question), item.options.length
                ? h("div", { class: "chat-chips" }, item.options.map((o) => h("button", { class: "chip", type: "button", on: { click: () => actions.onFollowUp(o.reply) } }, o.label)))
                : null);
        case "reject":
            return h("p", { class: "chat-reject" }, item.reason);
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
            items.push(h("h4", null, "Metodologia"), h("ul", null, item.items.map((text) => h("li", null, text))));
        if (item.type === "caveats")
            items.push(h("h4", null, "Cuidados"), h("ul", null, item.items.map((c) => h("li", null, c.text))));
        if (item.type === "sources")
            items.push(h("h4", null, "Fonte"), h("ul", null, item.items.map((src) => h("li", null, `${src.name}, competência ${src.competence}`, src.snapshot_sha256 ? h("code", { title: src.snapshot_sha256 }, ` snapshot ${src.snapshot_sha256.slice(0, 12)}…`) : null))));
    }
    return items.length ? h("details", { class: "chat-provenance" }, h("summary", null, "Como cheguei a esse resultado"), items) : null;
}
function extra(item) {
    if (item.type === "series")
        return LineChart(item.title, item.series);
    return records(item.title, item.columns, item.rows, item.total);
}
/** One assistant turn, rendered from the service's typed blocks. */
export function AnswerView(reply, question, actions) {
    const { answer } = reply;
    const blocks = answer.blocks;
    const kpis = blocks.filter((b) => b.type === "kpi");
    const charts = blocks.filter((b) => b.type === "chart");
    const rest = blocks.filter((b) => b.type !== "kpi" && b.type !== "chart");
    const firstText = blocks.findIndex((b) => b.type === "text");
    const total = reply.timing_us.total ?? 0;
    // Charts are titled by what was measured ("Estabelecimentos · padaria · Mato Grosso").
    const subject = answer.understood.split(" · ").filter((part) => !part.startsWith("RFB ")).join(" · ");
    const metric = subject ? subject.charAt(0).toUpperCase() + subject.slice(1) : "Resultado";
    const body = h("div", { class: "chat-answer" }, rest.filter((b) => b.type === "text" || b.type === "notice" || b.type === "clarify" || b.type === "reject").map((b) => block(b, actions, blocks.indexOf(b) === firstText)), kpis.length ? h("div", { class: "chat-kpis" }, kpis.map((b) => block(b, actions, false))) : null, reply.export ? exportCard(reply.export, question) : null, charts.map((b) => block(b, actions, false, metric)), reply.extras.filter((e) => e.type === "series").map(extra), rest.filter((b) => ["comparison", "table", "records", "company"].includes(b.type)).map((b) => block(b, actions, false)), reply.extras.filter((e) => e.type === "records").map(extra), provenance(blocks), answer.suggestions.length
        ? h("div", { class: "chat-chips chat-suggestions" }, answer.suggestions.map((text) => h("button", { class: "chip", type: "button", on: { click: () => actions.onFollowUp(text) } }, text)))
        : null, h("p", { class: "chat-meta" }, answer.understood ? `Entendi: ${answer.understood}` : null, total ? ` · respondido em ${formatValue(total / 1000)} ms` : null));
    return body;
}
//# sourceMappingURL=answer-view.js.map