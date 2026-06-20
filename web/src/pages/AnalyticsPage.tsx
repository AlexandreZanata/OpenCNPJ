import { useQuery } from '@tanstack/react-query'
import { statsPerCnae, statsPerCnaeAndUf, statsPerUf } from '../api/statsApi'
import { Card } from '../components/ui/Card'
import { ErrorState } from '../components/ui/ErrorState'
import { LoadingState } from '../components/ui/LoadingState'
import { formatNumber } from '../utils/format'

export function AnalyticsPage() {
  const ufStats = useQuery({ queryKey: ['stats', 'uf'], queryFn: statsPerUf, staleTime: 300_000 })
  const cnaeStats = useQuery({ queryKey: ['stats', 'cnae', 15], queryFn: () => statsPerCnae(15), staleTime: 300_000 })
  const topCnae = cnaeStats.data?.[0]?.cnae ?? ''
  const cnaeUfStats = useQuery({
    queryKey: ['stats', 'cnae-uf', topCnae],
    queryFn: () => statsPerCnaeAndUf(topCnae, 10),
    enabled: Boolean(topCnae),
    staleTime: 300_000,
  })

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Analytics</h1>
        <p className="text-slate-400">Aggregated CNAE and UF statistics from the API.</p>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card title="Estabelecimentos by UF">
          {ufStats.isLoading && <LoadingState />}
          {ufStats.error && <ErrorState message={(ufStats.error as Error).message} />}
          {ufStats.data && (
            <ul className="max-h-96 space-y-1 overflow-y-auto text-sm">
              {ufStats.data.map((row) => (
                <li key={row.uf} className="flex justify-between border-b border-border/40 py-2">
                  <span className="font-medium text-slate-200">{row.uf}</span>
                  <span className="text-slate-400">{formatNumber(row.count)}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="Top CNAE">
          {cnaeStats.isLoading && <LoadingState />}
          {cnaeStats.error && <ErrorState message={(cnaeStats.error as Error).message} />}
          {cnaeStats.data && (
            <ul className="space-y-1 text-sm">
              {cnaeStats.data.map((row) => (
                <li key={row.cnae} className="flex justify-between border-b border-border/40 py-2">
                  <span className="font-mono text-slate-300">{row.cnae}</span>
                  <span className="text-slate-400">{formatNumber(row.count)}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>

      {topCnae && (
        <Card title={`Top UF for CNAE ${topCnae}`}>
          {cnaeUfStats.isLoading && <LoadingState />}
          {cnaeUfStats.data && (
            <ul className="grid gap-2 sm:grid-cols-2 text-sm">
              {cnaeUfStats.data.map((row) => (
                <li key={row.uf} className="flex justify-between rounded-lg bg-slate-900/50 px-3 py-2">
                  <span>{row.uf}</span>
                  <span className="text-slate-400">{formatNumber(row.count)}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      )}
    </div>
  )
}
