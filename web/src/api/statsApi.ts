import { apiGet } from './client'
import type { StatsRow } from './types'

export function statsPerCnae(limit = 10): Promise<StatsRow[]> {
  return apiGet<StatsRow[]>('/stats/cnae', { limit })
}

export function statsPerUf(): Promise<StatsRow[]> {
  return apiGet<StatsRow[]>('/stats/uf')
}

export function statsPerCnaeAndUf(cnae: string, limit = 10): Promise<StatsRow[]> {
  return apiGet<StatsRow[]>(`/stats/cnae/${cnae}/uf`, { limit })
}
