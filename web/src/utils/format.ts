import type { SqlNullFloat, SqlNullString } from '../api/types'

export function unwrapString(value: SqlNullString | undefined): string {
  if (value == null) {
    return ''
  }
  if (typeof value === 'string') {
    return value
  }
  return value.Valid ? value.String : ''
}

export function unwrapFloat(value: SqlNullFloat | undefined): number | null {
  if (value == null) {
    return null
  }
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : null
  }
  if (typeof value === 'string') {
    const parsed = Number(value)
    return Number.isFinite(parsed) ? parsed : null
  }
  if (typeof value === 'object' && 'Valid' in value) {
    return value.Valid ? value.Float64 : null
  }
  return null
}

/** Parses ISO strings, YYYYMMDD, and Go sql.NullTime JSON objects. */
export function unwrapDate(value: unknown): Date | null {
  if (value == null) {
    return null
  }
  if (value instanceof Date) {
    return Number.isNaN(value.getTime()) ? null : value
  }
  if (typeof value === 'object' && 'Valid' in value) {
    const obj = value as { Time?: string; Valid: boolean }
    if (!obj.Valid) {
      return null
    }
    return parseDateInput(obj.Time ?? '')
  }
  if (typeof value === 'string') {
    return parseDateInput(value)
  }
  return null
}

function parseDateInput(raw: string): Date | null {
  const trimmed = raw.trim()
  if (!trimmed) {
    return null
  }
  if (/^\d{8}$/.test(trimmed)) {
    const year = Number(trimmed.slice(0, 4))
    const month = Number(trimmed.slice(4, 6)) - 1
    const day = Number(trimmed.slice(6, 8))
    const date = new Date(year, month, day)
    return Number.isNaN(date.getTime()) ? null : date
  }
  const parsed = new Date(trimmed)
  return Number.isNaN(parsed.getTime()) ? null : parsed
}

const dateFormatter = new Intl.DateTimeFormat('pt-BR', {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
  timeZone: 'UTC',
})

const dateTimeFormatter = new Intl.DateTimeFormat('pt-BR', {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
})

export function formatDisplayDate(value: unknown): string {
  const date = unwrapDate(value)
  return date ? dateFormatter.format(date) : ''
}

export function formatDisplayDateTime(value: unknown): string {
  const date = unwrapDate(value)
  return date ? dateTimeFormatter.format(date) : ''
}

export function formatNumber(value: number): string {
  return new Intl.NumberFormat('pt-BR').format(value)
}

export function formatCurrency(value: number | null): string {
  if (value == null) {
    return '—'
  }
  return new Intl.NumberFormat('pt-BR', { style: 'currency', currency: 'BRL' }).format(value)
}
