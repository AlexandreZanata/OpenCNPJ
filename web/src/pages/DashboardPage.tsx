import { useQuery } from '@tanstack/react-query'
import { statsPerCnae, statsPerUf } from '../api/statsApi'
import { CnpjSearchBar } from '../components/search/CnpjSearchBar'
import { Card } from '../components/ui/Card'
import { ErrorState } from '../components/ui/ErrorState'
import { LoadingState } from '../components/ui/LoadingState'
import { formatNumber } from '../utils/format'

export function DashboardPage() {
  const ufStats = useQuery({ queryKey: ['stats', 'uf'], queryFn: statsPerUf, staleTime: 300_000 })
  const cnaeStats = useQuery({ queryKey: ['stats', 'cnae', 5], queryFn: () => statsPerCnae(5), staleTime: 300_000 })

  const totalEstab = ufStats.data?.reduce((sum, row) => sum + row.count, 0) ?? 0

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Dashboard</h1>
        <p className="text-slate-400">Search 71M+ Estabelecimentos and 68M+ Empresas from Receita Federal open data.</p>
      </div>

      <Card title="Quick CNPJ Lookup">
        <CnpjSearchBar />
      </Card>

      <div className="grid gap-4 md:grid-cols-3">
        <Card title="Estabelecimentos">
          {ufStats.isLoading ? (
            <LoadingState label="Loading totals…" />
          ) : ufStats.error ? (
            <ErrorState message={(ufStats.error as Error).message} />
          ) : (
            <p className="text-3xl font-bold text-brand-500">{formatNumber(totalEstab)}</p>
          )}
        </Card>
        <Card title="States (UF)">
          <p className="text-3xl font-bold text-white">{ufStats.data?.length ?? '—'}</p>
        </Card>
        <Card title="Top CNAE tracked">
          <p className="text-3xl font-bold text-white">{cnaeStats.data?.[0]?.cnae ?? '—'}</p>
        </Card>
      </div>

      {cnaeStats.data && (
        <Card title="Top CNAE by Estabelecimento count">
          <ul className="space-y-2 text-sm">
            {cnaeStats.data.map((row) => (
              <li key={row.cnae} className="flex justify-between border-b border-border/50 py-2">
                <span className="font-mono text-slate-300">{row.cnae}</span>
                <span className="text-slate-400">{formatNumber(row.count)}</span>
              </li>
            ))}
          </ul>
        </Card>
      )}
    </div>
  )
}
