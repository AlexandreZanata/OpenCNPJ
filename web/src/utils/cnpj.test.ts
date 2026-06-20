import { describe, expect, it } from 'vitest'
import { formatCnpj, isValidCnpj, normalizeCnpjInput } from './cnpj'

describe('cnpj utils', () => {
  it('formats 14-digit CNPJ', () => {
    expect(formatCnpj('30834734000137')).toBe('30.834.734/0001-37')
  })

  it('validates known CNPJ check digits', () => {
    expect(isValidCnpj('30834734000137')).toBe(true)
    expect(isValidCnpj('00000000000000')).toBe(false)
  })

  it('normalizes user input', () => {
    expect(normalizeCnpjInput('30.834.734/0001-37')).toBe('30834734000137')
  })
})
