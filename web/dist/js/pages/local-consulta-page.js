import { h } from "../core/dom.js";
import { QueryBox } from "../features/query/query-box.js";
import { PageHero } from "../ui/page-hero.js";
const examples = [
    "Quantas filiais ativas de restaurantes existem em Cuiabá/MT?",
    "Quantos vidraceiros ativos existem em MT, exceto na cidade de Cuiabá/MT?",
    "Compare a densidade por 10 mil habitantes de lojinhas ativas entre Rondonópolis/MT e Jaciara/MT.",
    "Mostre a evolução mensal de registros de empresas em Cuiabá/MT em 2025.",
    "Distribua os estabelecimentos ativos no estado de Mato Grosso por divisão CNAE conforme o cadastro atual.",
];
const numbers = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 2 });
function resultView(reply) {
    const { execution, interpretation } = reply;
    if (!execution) {
        const title = interpretation.decision === "clarify" ? "A consulta precisa de esclarecimento" : "Consulta fora do escopo";
        return h("div", null, h("h2", null, title), h("p", null, interpretation.reason));
    }
    if (execution.status !== "ok") {
        const needsSnapshot = execution.error?.includes("supply --snapshot");
        const message = needsSnapshot
            ? "Esta consulta precisa da base de estabelecimentos. Neste servidor local estão disponíveis os dados agregados."
            : execution.error ?? "O motor local não conseguiu executar esta consulta.";
        return h("div", null, h("h2", null, "Consulta não executada"), h("p", null, message));
    }
    const op = interpretation.request?.op;
    const metric = op?.metric;
    const density = typeof metric === "object" && "per10k" in metric;
    const share = metric === "share" || (typeof metric === "object" && "active_change" in metric);
    const inhabitantsPer = typeof metric === "object" && "inhabitants_per" in metric;
    const format = (value, base = 0) => {
        if (density)
            return base > 0 ? `${numbers.format((value / base) * 10_000)} por 10 mil habitantes` : "Sem base populacional";
        if (share && base > 0)
            return `${numbers.format((value / base) * 100)}%`;
        if (inhabitantsPer)
            return base > 0 ? `${numbers.format(value / base)} habitantes por estabelecimento` : "Sem denominador";
        return numbers.format(value);
    };
    const rowLabel = (key) => {
        if (op?.grain === "month")
            return `${String((key % 12) + 1).padStart(2, "0")}/${Math.floor(key / 12)}`;
        if (typeof op?.dim === "object" && op.dim.cnae === "division")
            return `Divisão CNAE ${String(key).padStart(2, "0")}`;
        return String(key);
    };
    const { scalar, numerator, denominator, rows } = execution.value ?? {};
    let summary = null;
    if (typeof scalar === "number") {
        summary = h("p", null, h("strong", null, format(scalar)));
    }
    else if (typeof numerator === "number" && typeof denominator === "number") {
        summary = h("p", null, format(numerator, denominator));
    }
    const table = rows?.length ? h("table", { class: "data-table" }, h("thead", null, h("tr", null, h("th", { scope: "col" }, "Grupo"), h("th", { scope: "col" }, "Resultado"))), h("tbody", null, rows.map((row) => h("tr", null, h("th", { scope: "row" }, row.label ?? rowLabel(row.key)), h("td", null, format(row.value, row.base)))))) : rows ? h("p", null, "Nenhum grupo encontrado para este recorte.") : null;
    return h("div", null, h("h2", null, "Resultado da consulta"), execution.text ? h("p", null, execution.text) : null, summary, table, interpretation.notes.map((note) => h("p", null, note)), h("details", null, h("summary", null, "Ver filtros e dados da execução"), h("pre", { class: "local-query-details" }, JSON.stringify(reply, null, 2))));
}
/** Live preview backed by a local, same-origin OpenCNPJ engine. */
export function LocalConsultaPage() {
    const result = h("section", { class: "card query-box", hidden: true, "aria-live": "polite" });
    let pending = null;
    const submit = async (text) => {
        const question = text.trim();
        if (!question)
            return;
        pending?.abort();
        const request = new AbortController();
        pending = request;
        box.update({ value: question, loading: true });
        result.hidden = false;
        result.replaceChildren(h("p", null, "Consultando a base local…"));
        try {
            const response = await fetch("/api/local/ask", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ question }),
                signal: request.signal,
            });
            const reply = await response.json();
            if (!response.ok)
                throw new Error(reply.error ?? "Falha na consulta.");
            if (!request.signal.aborted)
                result.replaceChildren(resultView(reply));
        }
        catch (error) {
            if (!request.signal.aborted) {
                result.replaceChildren(h("p", { role: "alert" }, error instanceof Error ? error.message : "Falha na consulta."));
            }
        }
        finally {
            if (pending === request && !request.signal.aborted) {
                pending = null;
                const draft = box.el.querySelector("textarea")?.value ?? question;
                box.update({ value: draft, loading: false });
            }
        }
    };
    const clear = () => {
        pending?.abort();
        pending = null;
        box.update({ value: "", loading: false });
        result.hidden = true;
        result.replaceChildren();
    };
    const box = QueryBox({
        examples,
        onSubmit: (text) => void submit(text),
        onExample: (text) => void submit(text),
        onClear: clear,
    });
    box.update({ value: "", loading: false });
    return {
        el: h("div", { class: "page page--consulta page--local-consulta" }, PageHero({
            eyebrow: "Pesquisa com IA · motor local",
            title: ["Inteligência empresarial", "com dados oficiais do CNPJ"],
            lead: "Teste contagens, comparações e séries com os dados agregados da Receita Federal de outubro de 2026. Cada consulta é independente.",
        }), box.el, result),
        destroy: clear,
    };
}
//# sourceMappingURL=local-consulta-page.js.map