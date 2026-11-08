import { h, type View } from "../../core/dom.js";
import { formatInteger } from "../../core/format.js";
import type { Company, SortKey, SortState } from "../../domain/models.js";
import type { ExportFormat } from "../../services/export.js";
import { Button } from "../../ui/button.js";
import { Card } from "../../ui/card.js";
import { DataTable } from "../../ui/data-table.js";
import { Dropdown } from "../../ui/dropdown.js";
import { Pagination } from "../../ui/pagination.js";
import { COMPANY_COLUMNS, LOCKED_COLUMNS } from "./columns.js";

export interface CompaniesProps {
  rows: readonly Company[];
  total: number;
  page: number;
  pageSize: number;
  sort: SortState;
  loading: boolean;
  hiddenColumns: ReadonlySet<string>;
  filterCount: number;
}

export interface CompaniesOptions {
  onSort: (key: SortKey) => void;
  onPage: (page: number) => void;
  onToggleColumn: (id: string) => void;
  onExport: (format: ExportFormat) => void;
  onToggleFilters: () => void;
}

const SORT_LABEL: Record<SortKey, string> = {
  cnpj: "CNPJ",
  razaoSocial: "razão social",
  cnae: "CNAE principal",
  municipio: "município",
  capitalSocial: "capital social",
  abertura: "data de abertura",
  situacao: "situação",
};

function describeSort({ key, direction }: SortState): string {
  const order =
    key === "abertura"
      ? direction === "desc" ? "mais recentes" : "mais antigas"
      : direction === "asc" ? "crescente" : "decrescente";
  return `Ordenado por ${SORT_LABEL[key]} (${order})`;
}

/** "Empresas encontradas": toolbar, sortable table and pager. */
export function CompaniesSection(options: CompaniesOptions): View<CompaniesProps> {
  const table = DataTable<Company, SortKey>({ caption: "Empresas encontradas", onSort: options.onSort });
  const pager = Pagination(options.onPage);

  const filtersButton = Button({ label: "Filtros", icon: "filter", badge: 0, onClick: options.onToggleFilters });
  const columnsMenu = Dropdown({
    trigger: { label: "Colunas", icon: "columns" },
    items: [],
  });
  const exportMenu = Dropdown({
    trigger: { label: "Exportar", icon: "download", trailingIcon: "chevron-down" },
    items: [
      { id: "csv", label: "Exportar como CSV", onSelect: () => options.onExport("csv") },
      { id: "ndjson", label: "Exportar como NDJSON", onSelect: () => options.onExport("ndjson") },
    ],
  });

  const meta = h("p", { class: "companies__meta" });
  const summary = h("p", { class: "companies__summary", "aria-live": "polite" });

  const el = h(
    "section",
    { class: "companies", "aria-labelledby": "companies-title" },
    h(
      "div",
      { class: "companies__header" },
      h("div", null, h("h2", { class: "companies__title", id: "companies-title" }, "Empresas encontradas"), meta),
      h("div", { class: "companies__toolbar" }, filtersButton, columnsMenu.el, exportMenu.el),
    ),
    Card({ class: "companies__card" }, table.el),
    h("footer", { class: "companies__footer" }, summary, pager.el),
  );

  return {
    el,
    update: (props) => {
      const visible = COMPANY_COLUMNS.filter((c) => !props.hiddenColumns.has(c.id));
      table.update({
        columns: visible,
        rows: props.rows,
        sort: props.sort,
        loading: props.loading,
        rowKey: (row) => row.cnpj,
        emptyMessage: "Nenhuma empresa encontrada para esta consulta.",
      });

      const pageCount = Math.max(1, Math.ceil(props.total / props.pageSize));
      pager.update({ page: props.page, pageCount });

      const first = props.total === 0 ? 0 : (props.page - 1) * props.pageSize + 1;
      const last = Math.min(props.total, props.page * props.pageSize);
      meta.textContent = `${formatInteger(props.total)} registros · ${describeSort(props.sort)}`;
      summary.textContent = `Mostrando ${first} a ${last} de ${formatInteger(props.total)} resultados`;

      const badge = filtersButton.querySelector(".btn__badge");
      if (badge) {
        badge.textContent = String(props.filterCount);
        (badge as HTMLElement).hidden = props.filterCount === 0;
      }

      columnsMenu.update({
        items: COMPANY_COLUMNS.filter((c) => !LOCKED_COLUMNS.has(c.id)).map((c) => ({
          id: c.id,
          label: c.header,
          checked: !props.hiddenColumns.has(c.id),
          keepOpen: true,
          onSelect: () => options.onToggleColumn(c.id),
        })),
      });
    },
  };
}
