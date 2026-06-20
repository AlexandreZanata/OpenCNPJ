import { apiGet } from './client'
import type { LookupItem } from './types'

export function lookupSectors(q: string, limit = 15): Promise<LookupItem[]> {
  return apiGet<LookupItem[]>('/lookup/sectors', { q, limit })
}

export function lookupCnae(q: string, limit = 15): Promise<LookupItem[]> {
  return apiGet<LookupItem[]>('/lookup/cnae', { q, limit })
}

export function lookupMunicipio(q: string, uf = '', limit = 15): Promise<LookupItem[]> {
  return apiGet<LookupItem[]>('/lookup/municipio', { q, uf, limit })
}

export function lookupNomeFantasia(q: string, uf = '', limit = 15): Promise<LookupItem[]> {
  return apiGet<LookupItem[]>('/lookup/nome-fantasia', { q, uf, limit })
}

export function lookupUF(q: string): Promise<LookupItem[]> {
  return apiGet<LookupItem[]>('/lookup/uf', { q })
}
