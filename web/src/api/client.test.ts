import { describe, expect, it } from 'vitest'
import { ApiError } from './client'

describe('ApiError', () => {
  it('stores status and code', () => {
    const err = new ApiError(404, 'not_found', 'Estabelecimento not found')
    expect(err.status).toBe(404)
    expect(err.code).toBe('not_found')
    expect(err.message).toBe('Estabelecimento not found')
  })
})
