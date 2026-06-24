import type { ReactNode } from 'react'
import { formatCurrency, formatDisplayDate, formatDisplayDateTime, unwrapFloat, unwrapString } from '../../utils/format'

export type FieldFormat = 'date' | 'datetime' | 'currency'

export interface FieldDef {
  key: string
  label: string
  format?: FieldFormat
  render?: (value: unknown) => ReactNode
}

interface EntityFieldGridProps {
  fields: FieldDef[]
  data: Record<string, unknown>
}

export function EntityFieldGrid({ fields, data }: EntityFieldGridProps) {
  return (
    <dl className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {fields.map((field) => {
        const raw = data[field.key]
        const value = resolveFieldValue(field, raw)
        return (
          <div key={field.key}>
            <dt className="text-xs uppercase tracking-wide text-slate-500">{field.label}</dt>
            <dd className="mt-1 break-words text-sm text-slate-100">{value || '—'}</dd>
          </div>
        )
      })}
    </dl>
  )
}

function resolveFieldValue(field: FieldDef, raw: unknown): ReactNode {
  if (field.render) {
    return field.render(raw)
  }
  switch (field.format) {
    case 'date':
      return formatDisplayDate(raw)
    case 'datetime':
      return formatDisplayDateTime(raw)
    case 'currency':
      return formatCurrency(unwrapFloat(raw as never))
    default:
      return unwrapString(raw as never) || formatRaw(raw)
  }
}

function formatRaw(value: unknown): string {
  if (value == null) {
    return ''
  }
  if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
    return String(value)
  }
  if (typeof value === 'object' && 'Valid' in (value as object)) {
    return unwrapString(value as never)
  }
  return JSON.stringify(value)
}
