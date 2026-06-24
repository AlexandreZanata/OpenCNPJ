import { describe, expect, it } from 'vitest'
import { formatCurrency, formatDisplayDate, formatDisplayDateTime, unwrapDate, unwrapFloat, unwrapString } from './format'

describe('format utils', () => {
  it('unwraps Go sql.NullString objects', () => {
    expect(unwrapString({ String: 'ACME', Valid: true })).toBe('ACME')
    expect(unwrapString({ String: '', Valid: false })).toBe('')
    expect(unwrapString('plain')).toBe('plain')
  })

  it('unwraps Go sql.NullFloat64 objects', () => {
    expect(unwrapFloat({ Float64: 1000, Valid: true })).toBe(1000)
    expect(unwrapFloat({ Float64: 0, Valid: false })).toBeNull()
    expect(unwrapFloat(1500)).toBe(1500)
    expect(unwrapFloat(0)).toBe(0)
  })

  it('formats currency', () => {
    expect(formatCurrency(1500)).toContain('1')
  })

  it('parses ISO datetime and formats pt-BR', () => {
    const formatted = formatDisplayDateTime('2026-06-24T15:51:15.290648Z')
    expect(formatted).toMatch(/24/)
    expect(formatted).toMatch(/06/)
    expect(formatted).toMatch(/2026/)
  })

  it('parses YYYYMMDD RFB dates', () => {
    expect(formatDisplayDate('20150315')).toBe('15/03/2015')
  })

  it('parses Go sql.NullTime JSON', () => {
    expect(unwrapDate({ Time: '2024-03-15T00:00:00Z', Valid: true })).not.toBeNull()
    expect(unwrapDate({ Time: '0001-01-01T00:00:00Z', Valid: false })).toBeNull()
    expect(formatDisplayDate({ Time: '2024-03-15T00:00:00Z', Valid: true })).toBe('15/03/2024')
  })
})
