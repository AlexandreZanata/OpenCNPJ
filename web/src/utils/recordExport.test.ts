import { describe, expect, it } from 'vitest'
import { serializeRecordJson } from './recordExport'

describe('recordExport', () => {
  it('serializes records as pretty JSON', () => {
    const json = serializeRecordJson({ cnpj_basico: '12345678', razao_social: 'ACME' })
    expect(json).toContain('"cnpj_basico": "12345678"')
    expect(json).toContain('\n')
  })
})
