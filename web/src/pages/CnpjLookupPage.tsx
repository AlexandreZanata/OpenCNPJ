import { useQuery } from '@tanstack/react-query'
import { useParams } from 'react-router-dom'
import { getEstabelecimentoByCnpj } from '../api/estabelecimentoApi'
import { EstabelecimentoDetail } from '../components/detail/EstabelecimentoDetail'
import { CnpjSearchBar } from '../components/search/CnpjSearchBar'
import { Card } from '../components/ui/Card'
import { ErrorState } from '../components/ui/ErrorState'
import { LoadingState } from '../components/ui/LoadingState'
import { formatCnpj } from '../utils/cnpj'

export function CnpjLookupPage() {
  const { cnpj = '' } = useParams()
  const query = useQuery({
    queryKey: ['estabelecimento', cnpj],
    queryFn: () => getEstabelecimentoByCnpj(cnpj),
    enabled: cnpj.length === 14,
    staleTime: 300_000,
  })

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">CNPJ Lookup</h1>
        <p className="text-slate-400">Exact Estabelecimento lookup by 14-digit CNPJ.</p>
      </div>

      <Card title="Search">
        <CnpjSearchBar />
      </Card>

      {!cnpj && <p className="text-slate-500">Enter a CNPJ above to view establishment details.</p>}

      {cnpj && cnpj.length !== 14 && (
        <ErrorState title="Invalid CNPJ" message="CNPJ must contain exactly 14 digits." />
      )}

      {query.isLoading && <LoadingState label={`Loading ${formatCnpj(cnpj)}…`} />}
      {query.error && <ErrorState message={(query.error as Error).message} />}
      {query.data && <EstabelecimentoDetail data={query.data} />}
    </div>
  )
}
