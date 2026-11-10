/**
 * Search by filters over `GET /api/v1/search`. The filters live in the address
 * (`#/busca/uf=MT&cnae=4771701`), so a search can be bookmarked, shared and stepped
 * through with the browser's back button.
 */
import { config } from "../config.js";
import { errorMessage, getJson, request, saveAs } from "../core/api.js";
import { h, mount } from "../core/dom.js";
import { formatCnpj, formatInteger, formatRfbDate, formatRfbMoney } from "../core/format.js";
import { navigate } from "../core/router.js";
import { session } from "../core/session.js";
import { openCompany } from "../features/company/company-view.js";
import { Badge, Button, Checkbox, DataTable, EmptyState, Field, Input, Select, Spinner, busy, toast } from "../ui/kit.js";
const UFS = ["AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE", "PI", "PR", "RJ", "RN", "RO", "RR", "RS", "SC", "SE", "SP", "TO"];
let cnaes = null;
const cities = new Map();
function title(text) {
    return text.toLowerCase().replace(/(^|[\s\-/.(])(\p{L})/gu, (_, before, letter) => before + letter.toUpperCase());
}
/** Filters held in the address; `pagina` is the page shown. */
function read(param) {
    return new URLSearchParams(param);
}
function criteria(filters) {
    const query = new URLSearchParams(filters);
    query.delete("pagina");
    return query;
}
export function SearchPage(param) {
    const filters = read(param);
    const controller = new AbortController();
    const get = (key) => filters.get(key) ?? "";
    // ----- form ------------------------------------------------------------------------------
    const name = Input({ value: get("q"), placeholder: "Razão social, nome fantasia ou CNPJ" });
    const uf = Select([["", "Todos os estados"], ...UFS.map((code) => [code, code])], get("uf"));
    const city = Select([["", "Todas as cidades"]], "");
    city.disabled = true;
    const activity = Input({ value: get("cnae"), placeholder: "Código CNAE ou descrição" });
    activity.setAttribute("list", "cnae-options");
    const activityOptions = h("datalist", { id: "cnae-options" });
    const status = Select([
        ["02", "Ativas"],
        ["", "Todas as situações"],
        ["08", "Baixadas"],
        ["04", "Inaptas"],
        ["03", "Suspensas"],
    ], filters.has("situacao") || param ? get("situacao") : "02");
    const size = Select([
        ["", "Todos os portes"],
        ["01", "Microempresa"],
        ["03", "Pequeno porte"],
        ["05", "Demais (médias e grandes)"],
    ], get("porte"));
    const regime = Select([
        ["", "Qualquer regime"],
        ["mei", "MEI"],
        ["simples", "Simples Nacional"],
        ["fora", "Fora do Simples"],
    ], get("mei") === "S" ? "mei" : get("simples") === "S" ? "simples" : get("simples") === "N" ? "fora" : "");
    const kind = Select([
        ["", "Matrizes e filiais"],
        ["1", "Só matrizes"],
        ["2", "Só filiais"],
    ], get("matriz_filial"));
    const phone = Checkbox("Com telefone", get("tem_telefone") === "true");
    const email = Checkbox("Com e-mail", get("tem_email") === "true");
    const openedFrom = Input({ type: "date", value: get("data_inicio_atividade_de") });
    const openedTo = Input({ type: "date", value: get("data_inicio_atividade_ate") });
    const capitalMin = Input({ type: "number", value: get("capital_social_min"), placeholder: "0", inputmode: "numeric" });
    const order = Select([
        ["", "Ordem padrão"],
        ["data_inicio_atividade:desc", "Mais recentes primeiro"],
        ["data_inicio_atividade:asc", "Mais antigas primeiro"],
        ["capital_social:desc", "Maior capital primeiro"],
    ], get("ordenar"));
    const loadCities = (code, selected) => {
        city.replaceChildren(h("option", { value: "" }, code ? "Todas as cidades" : "Escolha o estado"));
        city.disabled = !code;
        if (!code)
            return;
        let list = cities.get(code);
        if (!list) {
            list = getJson(`/api/v1/dominios/municipios/${code}`);
            cities.set(code, list);
        }
        list
            .then((rows) => {
            if (uf.value !== code)
                return;
            city.append(...rows.map((row) => h("option", { value: row.codigo, selected: row.codigo === selected }, title(row.descricao))));
        })
            .catch(() => cities.delete(code));
    };
    uf.addEventListener("change", () => loadCities(uf.value, ""));
    loadCities(get("uf"), get("municipio"));
    cnaes ??= getJson("/api/v1/dominios/cnaes").catch(() => {
        cnaes = null;
        return [];
    });
    void cnaes.then((rows) => {
        activityOptions.append(...rows.map((row) => h("option", { value: `${row.codigo} — ${row.descricao}` })));
        // A code restored from the address is shown with its description.
        const known = rows.find((row) => row.codigo === activity.value.trim());
        if (known)
            activity.value = `${known.codigo} — ${known.descricao}`;
    });
    const collect = () => {
        const next = new URLSearchParams();
        const set = (key, value) => {
            if (value.trim())
                next.set(key, value.trim());
        };
        set("q", name.value);
        set("uf", uf.value);
        set("municipio", city.value);
        // "4771701 — Comércio varejista…" keeps the code; a plain code or prefix is used as typed.
        const code = /^[0-9A-Za-z]+/.exec(activity.value.trim())?.[0] ?? "";
        set("cnae", code);
        next.set("situacao", status.value);
        set("porte", size.value);
        if (regime.value === "mei")
            next.set("mei", "S");
        if (regime.value === "simples")
            next.set("simples", "S");
        if (regime.value === "fora")
            next.set("simples", "N");
        set("matriz_filial", kind.value);
        if (phone.firstElementChild.checked)
            next.set("tem_telefone", "true");
        if (email.firstElementChild.checked)
            next.set("tem_email", "true");
        set("data_inicio_atividade_de", openedFrom.value);
        set("data_inicio_atividade_ate", openedTo.value);
        set("capital_social_min", capitalMin.value);
        set("ordenar", order.value);
        if (get("cnpj"))
            next.set("cnpj", get("cnpj"));
        if (!next.get("situacao"))
            next.delete("situacao");
        return next;
    };
    const exportButton = Button({
        label: "Exportar CSV",
        icon: "download",
        onClick: () => void busy(exportButton, async () => {
            const query = criteria(filters);
            query.set("format", "csv");
            try {
                await saveAs(await request(`/api/v1/export?${query.toString()}`), "opencnpj-busca.csv");
            }
            catch (error) {
                toast(errorMessage(error), "error");
            }
        }),
    });
    exportButton.hidden = true;
    const form = h("form", {
        class: "filters card",
        on: {
            submit: (event) => {
                event.preventDefault();
                navigate("busca", collect().toString());
            },
        },
    }, h("div", { class: "filters__grid" }, Field({ label: "Empresa", control: name, class: "filters__wide" }), Field({ label: "Atividade (CNAE)", control: activity, class: "filters__wide" }), Field({ label: "Estado", control: uf }), Field({ label: "Cidade", control: city }), Field({ label: "Situação", control: status }), Field({ label: "Porte", control: size }), Field({ label: "Regime", control: regime }), Field({ label: "Tipo", control: kind }), Field({ label: "Aberta a partir de", control: openedFrom }), Field({ label: "Aberta até", control: openedTo }), Field({ label: "Capital mínimo (R$)", control: capitalMin }), Field({ label: "Ordenar", control: order })), activityOptions, h("div", { class: "filters__foot" }, h("div", { class: "filters__checks" }, phone, email), h("div", { class: "actions" }, Button({ label: "Limpar", variant: "ghost", onClick: () => navigate("busca") }), exportButton, Button({ label: "Buscar", icon: "search", variant: "primary", type: "submit" }))));
    // ----- results ---------------------------------------------------------------------------
    const results = h("div", { class: "page__result" });
    const page = Math.max(1, Number(get("pagina")) || 1);
    const platform = session().platform;
    const trial = !session().token && platform?.auth === "trial";
    const pageSize = Math.min(config.pageSize, session().account?.plan.max_page_size ?? (trial ? platform.trial?.max_page_size ?? 20 : config.pageSize));
    const run = () => {
        const query = criteria(filters);
        query.set("limit", String(pageSize));
        query.set("offset", String((page - 1) * pageSize));
        results.replaceChildren(Spinner("Buscando"));
        getJson(`/api/v1/search?${query.toString()}`, controller.signal)
            .then((reply) => {
            if (reply.total === 0) {
                results.replaceChildren(EmptyState("search", "Nenhum estabelecimento com esses filtros.", "Tire um filtro ou amplie a região."));
                return;
            }
            const pages = Math.ceil(reply.total / pageSize);
            const go = (to) => {
                const next = new URLSearchParams(filters);
                next.set("pagina", String(to));
                navigate("busca", next.toString());
            };
            exportButton.hidden = trial || session().account?.plan.export === false;
            mount(results, h("div", { class: "results__head" }, h("strong", null, `${formatInteger(reply.total)} ${reply.total === 1 ? "estabelecimento" : "estabelecimentos"}`), h("span", { class: "muted" }, `${reply.fuzzy_used ? "nomes parecidos · " : ""}${reply.took_ms.total.toFixed(1).replace(".", ",")} ms`)), DataTable([
                { header: "CNPJ", cell: (row) => h("span", { class: "mono" }, formatCnpj(row.cnpj)) },
                {
                    header: "Empresa",
                    cell: (row) => [h("strong", null, title(row.razao_social)), row.nome_fantasia ? h("span", { class: "cell-sub" }, title(row.nome_fantasia)) : null],
                    class: "cell-main",
                },
                { header: "Cidade", cell: (row) => `${title(row.municipio_descricao ?? "")}/${row.uf}` },
                { header: "Atividade", cell: (row) => row.cnae_fiscal_principal_descricao, class: "cell-wrap" },
                { header: "Abertura", cell: (row) => formatRfbDate(row.data_inicio_atividade) },
                { header: "Capital", cell: (row) => formatRfbMoney(row.capital_social), class: "num" },
                {
                    header: "Situação",
                    cell: (row) => Badge(title(row.situacao_cadastral_descricao ?? ""), row.situacao_cadastral === "02" ? "good" : row.situacao_cadastral === "08" ? "bad" : "warn"),
                },
            ], reply.results, (row) => openCompany(row.cnpj)), pages > 1
                ? h("nav", { class: "pager", "aria-label": "Páginas" }, Button({ label: "Anterior", icon: "chevron-left", size: "sm", disabled: page <= 1, onClick: () => go(page - 1) }), h("span", { class: "muted" }, `Página ${formatInteger(page)} de ${formatInteger(pages)}`), Button({ label: "Próxima", size: "sm", disabled: page >= pages, onClick: () => go(page + 1) }))
                : null);
        })
            .catch((error) => {
            const message = errorMessage(error);
            if (message)
                results.replaceChildren(EmptyState("info", message));
        });
    };
    const hasCriteria = [...criteria(filters).keys()].some((key) => key !== "situacao" && key !== "ordenar");
    if (hasCriteria)
        run();
    else
        results.replaceChildren(EmptyState("filter", "Escolha ao menos um filtro para buscar.", "Por exemplo: um estado e uma atividade, ou parte do nome da empresa."));
    return {
        el: h("div", { class: "page page--wide" }, h("h1", { class: "page__title" }, "Busca por filtros"), form, results),
        destroy: () => controller.abort(),
    };
}
//# sourceMappingURL=search-page.js.map