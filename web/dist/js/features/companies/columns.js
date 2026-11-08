import { h } from "../../core/dom.js";
import { formatBRL, formatCnpj, formatDate } from "../../core/format.js";
import { apiUrl } from "../../config.js";
import { Badge } from "../../ui/badge.js";
import { Icon } from "../../ui/icon.js";
const SITUACAO_TONE = {
    Ativa: "success",
    Suspensa: "warning",
    Inapta: "danger",
    Baixada: "neutral",
    Nula: "neutral",
};
/** Columns that can never be hidden. */
export const LOCKED_COLUMNS = new Set(["open", "cnpj"]);
export const COMPANY_COLUMNS = [
    {
        id: "open",
        header: "",
        width: 4.9,
        render: (row) => h("a", {
            class: "row-link",
            href: apiUrl(`/api/v1/cnpj/${row.cnpj}`),
            target: "_blank",
            rel: "noopener noreferrer",
            "aria-label": `Abrir ${formatCnpj(row.cnpj)} na API`,
        }, Icon({ name: "external-link", size: 15, strokeWidth: 1.9 })),
    },
    { id: "cnpj", header: "CNPJ", width: 11.6, sortKey: "cnpj", render: (row) => formatCnpj(row.cnpj) },
    { id: "razaoSocial", header: "Razão Social", width: 20.6, sortKey: "razaoSocial", render: (row) => row.razaoSocial },
    {
        id: "cnae",
        header: "CNAE Principal",
        width: 23,
        sortKey: "cnae",
        render: (row) => `${row.cnaeCodigo} — ${row.cnaeDescricao}`,
    },
    { id: "municipio", header: "Município / UF", width: 13.2, sortKey: "municipio", render: (row) => `${row.municipio} / ${row.uf}` },
    { id: "capitalSocial", header: "Capital Social", width: 10.6, sortKey: "capitalSocial", render: (row) => formatBRL(row.capitalSocial) },
    { id: "abertura", header: "Abertura", width: 9.9, sortKey: "abertura", render: (row) => formatDate(row.abertura) },
    { id: "situacao", header: "Situação", width: 6.2, sortKey: "situacao", render: (row) => Badge({ label: row.situacao, tone: SITUACAO_TONE[row.situacao] }) },
];
//# sourceMappingURL=columns.js.map