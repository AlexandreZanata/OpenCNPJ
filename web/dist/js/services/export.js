const HEADER = ["cnpj", "razao_social", "cnae_principal", "cnae_descricao", "municipio", "uf", "capital_social", "data_abertura", "situacao"];
function toRecord(company) {
    return {
        cnpj: company.cnpj,
        razao_social: company.razaoSocial,
        cnae_principal: company.cnaeCodigo,
        cnae_descricao: company.cnaeDescricao,
        municipio: company.municipio,
        uf: company.uf,
        capital_social: company.capitalSocial,
        data_abertura: company.abertura,
        situacao: company.situacao,
    };
}
function csvCell(value) {
    const text = String(value);
    return /[",\n;]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}
export function serialize(rows, format) {
    const records = rows.map(toRecord);
    if (format === "ndjson")
        return records.map((r) => JSON.stringify(r)).join("\n") + "\n";
    const lines = records.map((r) => HEADER.map((key) => csvCell(r[key])).join(","));
    return [HEADER.join(","), ...lines].join("\n") + "\n";
}
/** Triggers a browser download for `content`. */
export function download(filename, content, mime) {
    const url = URL.createObjectURL(new Blob([content], { type: `${mime};charset=utf-8` }));
    const link = document.createElement("a");
    link.href = url;
    link.download = filename;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
}
//# sourceMappingURL=export.js.map