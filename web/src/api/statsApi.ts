import { apiGet } from './client'
import type { StatsRow } from './types'

export interface AnalyticsSummary {
  source: string
  refreshed_at?: string
  by_uf: StatsRow[]
  top_cnae: StatsRow[]
  top_cnae_uf: {
    cnae: string
    by_uf: StatsRow[]
  }
}

export function getAnalyticsSummary(cnaeLimit = 15, cnaeUfLimit = 10): Promise<AnalyticsSummary> {
  return apiGet<AnalyticsSummary>('/analytics/summary', {
    cnae_limit: cnaeLimit,
    cnae_uf_limit: cnaeUfLimit,
  })
}

export function statsPerCnae(limit = 10): Promise<StatsRow[]> {
  return apiGet<StatsRow[]>('/stats/cnae', { limit })
}

export function statsPerUf(): Promise<StatsRow[]> {
  return apiGet<StatsRow[]>('/stats/uf')
}

export function statsPerCnaeAndUf(cnae: string, limit = 10): Promise<StatsRow[]> {
  return apiGet<StatsRow[]>(`/stats/cnae/${cnae}/uf`, { limit })
}
