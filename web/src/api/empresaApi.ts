import { apiGet } from './client'
import type { Empresa, EmpresaSearchParams, SearchResponse } from './types'

export function searchEmpresas(params: EmpresaSearchParams): Promise<SearchResponse<Empresa>> {
  return apiGet<SearchResponse<Empresa>>('/empresas/search', params as Record<string, string | number | undefined>)
}
