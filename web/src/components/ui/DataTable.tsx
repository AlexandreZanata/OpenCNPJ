import type { ReactNode } from 'react'

export interface Column<T> {
  key: string
  header: string
  render: (row: T) => ReactNode
}

interface DataTableProps<T> {
  columns: Column<T>[]
  rows: T[]
  emptyMessage?: string
}

export function DataTable<T>({ columns, rows, emptyMessage = 'No results found.' }: DataTableProps<T>) {
  if (rows.length === 0) {
    return <p className="py-8 text-center text-slate-400">{emptyMessage}</p>
  }

  return (
    <div className="overflow-x-auto rounded-lg border border-border">
      <table className="min-w-full divide-y divide-border text-sm">
        <thead className="bg-slate-900/80">
          <tr>
            {columns.map((column) => (
              <th key={column.key} className="px-4 py-3 text-left font-semibold text-slate-300">
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-border/70">
          {rows.map((row, index) => (
            <tr key={index} className="hover:bg-slate-900/40">
              {columns.map((column) => (
                <td key={column.key} className="px-4 py-3 text-slate-200">
                  {column.render(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
