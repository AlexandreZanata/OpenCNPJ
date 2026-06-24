import { describe, expect, it } from 'vitest'
import { resolveBrazilianUF } from './uf'

describe('resolveBrazilianUF', () => {
  it('uses selected item code', () => {
    expect(resolveBrazilianUF({ type: 'uf', code: 'pr', label: 'PR — Paraná' }, '')).toBe('PR')
  })

  it('parses two-letter typed code', () => {
    expect(resolveBrazilianUF(null, 'sp')).toBe('SP')
  })

  it('parses label prefix', () => {
    expect(resolveBrazilianUF(null, 'PR — Paraná')).toBe('PR')
  })

  it('returns undefined for partial state name', () => {
    expect(resolveBrazilianUF(null, 'Paraná')).toBeUndefined()
  })
})
