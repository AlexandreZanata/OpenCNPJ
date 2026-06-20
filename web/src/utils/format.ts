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
    return value
  }
  return value.Valid ? value.Float64 : null
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
