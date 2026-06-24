import { apiGet } from './client'
import type { EmpresaAggregate, EmpresaSearchParams, SearchResponse } from './types'

export function searchEmpresas(params: EmpresaSearchParams): Promise<SearchResponse<EmpresaAggregate>> {
  return apiGet<SearchResponse<EmpresaAggregate>>('/empresas/search', params as Record<string, string | number | undefined>)
}
