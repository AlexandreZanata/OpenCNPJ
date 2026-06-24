import { apiGet } from './client'
import type { EstabelecimentoSearchParams, EstabelecimentoSearchResult, SearchResponse } from './types'

export function searchEstabelecimentos(
  params: EstabelecimentoSearchParams,
): Promise<SearchResponse<EstabelecimentoSearchResult>> {
  return apiGet<SearchResponse<EstabelecimentoSearchResult>>(
    '/estabelecimentos/search',
    params as Record<string, string | number | undefined>,
  )
}

export function getEstabelecimentoByCnpj(cnpj: string): Promise<EstabelecimentoSearchResult> {
  return apiGet<EstabelecimentoSearchResult>(`/estabelecimentos/${cnpj}`)
}
