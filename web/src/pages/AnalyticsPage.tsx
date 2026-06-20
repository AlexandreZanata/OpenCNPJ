import { useQuery } from '@tanstack/react-query'
import { getAnalyticsSummary } from '../api/statsApi'
import { Card } from '../components/ui/Card'
import { ErrorState } from '../components/ui/ErrorState'
import { LoadingState } from '../components/ui/LoadingState'
import { formatNumber } from '../utils/format'

export function AnalyticsPage() {
  const query = useQuery({
    queryKey: ['analytics', 'summary'],
    queryFn: () => getAnalyticsSummary(15, 10),
    staleTime: 600_000,
  })

  const data = query.data

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Analytics</h1>
        <p className="text-slate-400">
          Pre-aggregated CNAE and UF statistics
          {data?.refreshed_at && (
            <span className="text-slate-500"> · refreshed {new Date(data.refreshed_at).toLocaleString()}</span>
          )}
        </p>
      </div>

      {query.isLoading && <LoadingState label="Loading analytics summary…" />}
      {query.error && <ErrorState message={(query.error as Error).message} />}

      {data && (
        <div className="grid gap-6 lg:grid-cols-2">
          <Card title="Estabelecimentos by UF">
            <ul className="max-h-96 space-y-1 overflow-y-auto text-sm">
              {data.by_uf.map((row) => (
                <li key={row.uf} className="flex justify-between border-b border-border/40 py-2">
                  <span className="font-medium text-slate-200">{row.uf}</span>
                  <span className="text-slate-400">{formatNumber(row.count)}</span>
                </li>
              ))}
            </ul>
          </Card>

          <Card title="Top CNAE">
            <ul className="space-y-1 text-sm">
              {data.top_cnae.map((row) => (
                <li key={row.cnae} className="flex justify-between border-b border-border/40 py-2">
                  <span className="font-mono text-slate-300">{row.cnae}</span>
                  <span className="text-slate-400">{formatNumber(row.count)}</span>
                </li>
              ))}
            </ul>
          </Card>
        </div>
      )}

      {data?.top_cnae_uf.cnae && (
        <Card title={`Top UF for CNAE ${data.top_cnae_uf.cnae}`}>
          <ul className="grid gap-2 sm:grid-cols-2 text-sm">
            {data.top_cnae_uf.by_uf.map((row) => (
              <li key={row.uf} className="flex justify-between rounded-lg bg-slate-900/50 px-3 py-2">
                <span>{row.uf}</span>
                <span className="text-slate-400">{formatNumber(row.count)}</span>
              </li>
            ))}
          </ul>
        </Card>
      )}
    </div>
  )
}
