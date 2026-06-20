import { useQuery } from '@tanstack/react-query'
import { getAnalyticsSummary } from '../api/statsApi'
import { CnpjSearchBar } from '../components/search/CnpjSearchBar'
import { Card } from '../components/ui/Card'
import { ErrorState } from '../components/ui/ErrorState'
import { LoadingState } from '../components/ui/LoadingState'
import { formatNumber } from '../utils/format'

export function DashboardPage() {
  const analytics = useQuery({
    queryKey: ['analytics', 'summary', 5],
    queryFn: () => getAnalyticsSummary(5, 5),
    staleTime: 600_000,
  })

  const totalEstab = analytics.data?.by_uf.reduce((sum, row) => sum + row.count, 0) ?? 0

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Dashboard</h1>
        <p className="text-slate-400">Search 71M+ Estabelecimentos and 68M+ Empresas from Receita Federal open data.</p>
      </div>

      <Card title="Quick CNPJ Lookup">
        <CnpjSearchBar />
      </Card>

      {analytics.isLoading && <LoadingState label="Loading statistics…" />}
      {analytics.error && <ErrorState message={(analytics.error as Error).message} />}

      {analytics.data && (
        <>
          <div className="grid gap-4 md:grid-cols-3">
            <Card title="Estabelecimentos">
              <p className="text-3xl font-bold text-brand-500">{formatNumber(totalEstab)}</p>
            </Card>
            <Card title="States (UF)">
              <p className="text-3xl font-bold text-white">{analytics.data.by_uf.length}</p>
            </Card>
            <Card title="Top CNAE">
              <p className="font-mono text-3xl font-bold text-white">{analytics.data.top_cnae[0]?.cnae ?? '—'}</p>
            </Card>
          </div>

          <Card title="Top CNAE by Estabelecimento count">
            <ul className="space-y-2 text-sm">
              {analytics.data.top_cnae.map((row) => (
                <li key={row.cnae} className="flex justify-between border-b border-border/50 py-2">
                  <span className="font-mono text-slate-300">{row.cnae}</span>
                  <span className="text-slate-400">{formatNumber(row.count)}</span>
                </li>
              ))}
            </ul>
          </Card>
        </>
      )}
    </div>
  )
}
