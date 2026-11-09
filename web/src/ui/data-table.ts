import { cx, h, type Child, type View } from "../core/dom.js";
import type { SortDirection } from "../domain/models.js";
import { Icon } from "./icon.js";

export interface Column<Row, Key extends string> {
  id: string;
  header: string;
  /** Column width as a percentage of the table. */
  width: number;
  /** When set, the header becomes a sort button for this key. */
  sortKey?: Key;
  align?: "start" | "end";
  render: (row: Row) => Child;
}

export interface DataTableProps<Row, Key extends string> {
  columns: readonly Column<Row, Key>[];
  rows: readonly Row[];
  sort?: { key: Key; direction: SortDirection };
  loading?: boolean;
  rowKey: (row: Row) => string;
  emptyMessage?: string;
}

export interface DataTableOptions<Key extends string> {
  caption: string;
  onSort?: (key: Key) => void;
}

const ARIA_SORT: Record<SortDirection, string> = { asc: "ascending", desc: "descending" };

/** Generic, sortable table. Columns are data, so any list screen can reuse it. */
export function DataTable<Row, Key extends string>(
  options: DataTableOptions<Key>,
): View<DataTableProps<Row, Key>> {
  const colgroup = h("colgroup");
  const head = h("thead");
  const body = h("tbody");
  const table = h("table", { class: "data-table" }, h("caption", { class: "sr-only" }, options.caption), colgroup, head, body);
  const el = h("div", { class: "data-table__wrap" }, table);

  const update = ({ columns, rows, sort, loading, rowKey, emptyMessage }: DataTableProps<Row, Key>): void => {
    const totalWidth = columns.reduce((sum, c) => sum + c.width, 0) || 1;
    colgroup.replaceChildren(...columns.map((c) => h("col", { style: `width:${((c.width / totalWidth) * 100).toFixed(3)}%` })));

    head.replaceChildren(
      h(
        "tr",
        null,
        columns.map((column) => {
          const active = column.sortKey !== undefined && sort?.key === column.sortKey;
          const th = h("th", {
            scope: "col",
            class: cx(column.align === "end" && "is-end"),
            "aria-sort": column.sortKey ? (active && sort ? ARIA_SORT[sort.direction] : "none") : undefined,
          });
          if (column.sortKey === undefined) {
            if (column.header) th.append(column.header);
            return th;
          }
          const key = column.sortKey;
          th.append(
            h(
              "button",
              { class: cx("data-table__sort", active && "is-active"), type: "button", on: { click: () => options.onSort?.(key) } },
              h("span", null, column.header),
              Icon({ name: "chevrons-up-down", size: 13, class: "data-table__sort-icon" }),
            ),
          );
          return th;
        }),
      ),
    );

    body.replaceChildren(
      ...(rows.length === 0 && !loading
        ? [h("tr", null, h("td", { class: "data-table__empty", colspan: columns.length }, emptyMessage ?? "Nenhum resultado."))]
        : rows.map((row) =>
            h(
              "tr",
              { "data-key": rowKey(row) },
              columns.map((column) => h("td", { class: cx(column.align === "end" && "is-end") }, column.render(row))),
            ),
          )),
    );

    table.setAttribute("aria-busy", String(Boolean(loading)));
    el.classList.toggle("is-loading", Boolean(loading));
  };

  return { el, update };
}
