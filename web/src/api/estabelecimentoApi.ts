import { apiGet } from './client'
import type { Estabelecimento, EstabelecimentoSearchParams, SearchResponse } from './types'

export function searchEstabelecimentos(
  params: EstabelecimentoSearchParams,
): Promise<SearchResponse<Estabelecimento>> {
  return apiGet<SearchResponse<Estabelecimento>>(
    '/estabelecimentos/search',
    params as Record<string, string | number | undefined>,
  )
}

export function getEstabelecimentoByCnpj(cnpj: string): Promise<Estabelecimento> {
  return apiGet<Estabelecimento>(`/estabelecimentos/${cnpj}`)
}
