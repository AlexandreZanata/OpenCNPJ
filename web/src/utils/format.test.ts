import { describe, expect, it } from 'vitest'
import { formatCurrency, unwrapFloat, unwrapString } from './format'

describe('format utils', () => {
  it('unwraps Go sql.NullString objects', () => {
    expect(unwrapString({ String: 'ACME', Valid: true })).toBe('ACME')
    expect(unwrapString({ String: '', Valid: false })).toBe('')
    expect(unwrapString('plain')).toBe('plain')
  })

  it('unwraps Go sql.NullFloat64 objects', () => {
    expect(unwrapFloat({ Float64: 1000, Valid: true })).toBe(1000)
    expect(unwrapFloat({ Float64: 0, Valid: false })).toBeNull()
  })

  it('formats currency', () => {
    expect(formatCurrency(1500)).toContain('1')
  })
})
