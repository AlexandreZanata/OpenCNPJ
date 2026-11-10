/** Registration record of one establishment (`GET /api/v1/cnpj/{cnpj}`), as a page or a drawer. */
import { errorMessage, getJson } from "../../core/api.js";
import { h } from "../../core/dom.js";
import { formatCep, formatCnpj, formatInteger, formatPhone, formatRfbDate, formatRfbMoney } from "../../core/format.js";
import { href } from "../../core/router.js";
import { Badge, Button, EmptyState, Spinner, copyText, openDrawer } from "../../ui/kit.js";
function statusTone(code) {
    if (code === "02")
        return "good";
    if (code === "08")
        return "bad";
    return code ? "warn" : "neutral";
}
function title(text) {
    return (text ?? "").toLowerCase().replace(/(^|[\s\-/.(])(\p{L})/gu, (_, before, letter) => before + letter.toUpperCase());
}
function list(items) {
    return h("dl", { class: "facts" }, items.filter(([, value]) => value !== "" && value !== null && value !== undefined && value !== false).map(([key, value]) => [h("dt", null, key), h("dd", null, value)]));
}
function section(name, body) {
    return h("section", { class: "record__section" }, h("h3", null, name), body);
}
function yes(flag, since) {
    if (flag !== "S")
        return "Não";
    const date = formatRfbDate(since);
    return date ? `Sim, desde ${date}` : "Sim";
}
/** The full record. `est` is the establishment looked up, or the first one of a company root. */
export function CompanyView(record) {
    const est = record.estabelecimento ?? record.estabelecimentos?.[0] ?? {};
    const company = record.empresa;
    const cnpj = record.cnpj ?? est["cnpj"] ?? "";
    const street = [title(`${est["tipo_logradouro"] ?? ""} ${est["logradouro"] ?? ""}`.trim()), est["numero"], title(est["complemento"])].filter(Boolean).join(", ");
    const city = [title(est["municipio_descricao"]), est["uf"]].filter(Boolean).join("/");
    const phones = [formatPhone(est["ddd_1"], est["telefone_1"]), formatPhone(est["ddd_2"], est["telefone_2"])].filter(Boolean);
    const email = (est["correio_eletronico"] ?? "").toLowerCase();
    const secondary = (est["cnae_fiscal_secundaria"] ?? "").split(",").map((code) => code.trim()).filter(Boolean);
    const partners = record.socios ?? [];
    const formatted = record.cnpj_formatado ?? formatCnpj(cnpj);
    return h("article", { class: "record" }, h("header", { class: "record__head" }, h("div", null, h("h2", { class: "record__name" }, title(company["razao_social"]) || "Empresa"), est["nome_fantasia"] ? h("p", { class: "muted" }, title(est["nome_fantasia"])) : null), h("div", { class: "record__tags" }, Badge(title(est["situacao_cadastral_descricao"]) || "Situação não informada", statusTone(est["situacao_cadastral"])), est["identificador_matriz_filial_descricao"] ? Badge(title(est["identificador_matriz_filial_descricao"])) : null)), h("div", { class: "record__id" }, h("span", { class: "mono" }, formatted), Button({ icon: "copy", variant: "ghost", size: "sm", title: "Copiar CNPJ", onClick: () => void copyText(formatted, "CNPJ copiado.") })), section("Cadastro", list([
        ["Abertura", formatRfbDate(est["data_inicio_atividade"])],
        ["Situação desde", formatRfbDate(est["data_situacao_cadastral"])],
        ["Motivo da situação", est["situacao_cadastral"] === "02" ? "" : title(est["motivo_situacao_cadastral_descricao"])],
        ["Natureza jurídica", company["natureza_juridica_descricao"] ?? ""],
        ["Porte", title(company["porte_empresa_descricao"])],
        ["Capital social", formatRfbMoney(company["capital_social"])],
        ["Simples Nacional", record.simples ? yes(record.simples["opcao_pelo_simples"], record.simples["data_opcao_simples"]) : ""],
        ["MEI", record.simples ? yes(record.simples["opcao_mei"], record.simples["data_opcao_mei"]) : ""],
    ])), section("Atividade", list([
        ["Principal", [h("span", { class: "mono" }, est["cnae_fiscal_principal"] ?? ""), " ", est["cnae_fiscal_principal_descricao"] ?? ""]],
        ["Secundárias", secondary.length ? h("span", { class: "mono" }, secondary.join(" · ")) : ""],
    ])), section("Endereço e contato", list([
        ["Endereço", street],
        ["Bairro", title(est["bairro"])],
        ["Cidade", city],
        ["CEP", formatCep(est["cep"])],
        ["Telefone", phones.length ? phones.map((phone) => h("a", { href: `tel:${phone.replace(/\D/g, "")}`, class: "record__contact" }, phone)) : ""],
        ["E-mail", email ? h("a", { href: `mailto:${email}`, class: "record__contact" }, email) : ""],
    ])), partners.length
        ? section(`Quadro societário (${formatInteger(partners.length)})`, h("ul", { class: "partners" }, partners.map((partner) => h("li", null, h("strong", null, title(partner["nome_socio_razao_social"])), h("span", { class: "muted" }, [partner["qualificacao_socio_descricao"], formatRfbDate(partner["data_entrada_sociedade"]) && `desde ${formatRfbDate(partner["data_entrada_sociedade"])}`].filter(Boolean).join(" · "))))))
        : null, record.total_estabelecimentos > 1
        ? h("p", { class: "record__more" }, `Esta empresa tem ${formatInteger(record.total_estabelecimentos)} estabelecimentos. `, h("a", { href: href("busca", `cnpj=${record.cnpj_basico}`) }, "Ver todos na busca"))
        : null, h("p", { class: "record__source muted" }, `Receita Federal — Dados Abertos do CNPJ${record.snapshot?.competence ? `, competência ${record.snapshot.competence}` : ""}.`));
}
/** Opens the record of `cnpj` in a side panel; the page behind stays as it is. */
export function openCompany(cnpj) {
    const digits = cnpj.replace(/[^0-9A-Za-z]/g, "");
    const body = h("div", { class: "record-host" }, Spinner("Buscando o CNPJ"));
    openDrawer("Ficha da empresa", body);
    getJson(`/api/v1/cnpj/${encodeURIComponent(digits)}`)
        .then((record) => body.replaceChildren(CompanyView(record)))
        .catch((error) => body.replaceChildren(EmptyState("info", errorMessage(error) || "Não consegui abrir este CNPJ.")));
}
//# sourceMappingURL=company-view.js.map