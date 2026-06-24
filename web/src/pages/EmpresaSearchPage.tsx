import { useQuery } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { searchEmpresas } from '../api/empresaApi'
import { EmpresaAggregateCard } from '../components/detail/EmpresaAggregateCard'
import { ExportPanel } from '../components/export/ExportPanel'
import { Card } from '../components/ui/Card'
import { ErrorState } from '../components/ui/ErrorState'
import { Input } from '../components/ui/Input'
import { LoadingState } from '../components/ui/LoadingState'
import { Pagination } from '../components/ui/Pagination'
import { useDebounce } from '../hooks/useDebounce'

const PAGE_SIZE = 5

export function EmpresaSearchPage() {
  const [cnpjBasico, setCnpjBasico] = useState('')
  const [razaoSocial, setRazaoSocial] = useState('')
  const [offset, setOffset] = useState(0)
  const debouncedRazao = useDebounce(razaoSocial, 500)

  const params = useMemo(
    () => ({
      cnpj_basico: cnpjBasico || undefined,
      razao_social: debouncedRazao || undefined,
      limit: PAGE_SIZE,
      offset,
    }),
    [cnpjBasico, debouncedRazao, offset],
  )

  const query = useQuery({
    queryKey: ['empresas', params],
    queryFn: () => searchEmpresas(params),
    enabled: Boolean(params.cnpj_basico || params.razao_social),
    staleTime: 120_000,
  })

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Empresa Search</h1>
        <p className="text-slate-400">
          Full empresa record: legal data, all branches (estabelecimentos), partners (sócios), and Simples/MEI.
        </p>
      </div>

      <Card title="Filters">
        <div className="grid gap-4 md:grid-cols-2">
          <Input label="CNPJ Básico" value={cnpjBasico} onChange={(e) => { setOffset(0); setCnpjBasico(e.target.value.replace(/\D/g, '').slice(0, 8)) }} placeholder="12345678" />
          <Input label="Razão Social" value={razaoSocial} onChange={(e) => { setOffset(0); setRazaoSocial(e.target.value) }} placeholder="LTDA, MERCADO…" />
        </div>
      </Card>

      {!params.cnpj_basico && !params.razao_social && (
        <p className="text-slate-500">Provide CNPJ básico or razão social to search.</p>
      )}

      {query.isLoading && <LoadingState />}
      {query.error && <ErrorState message={(query.error as Error).message} />}

      {query.data && (
        <>
          <Pagination
            offset={query.data.offset}
            limit={query.data.limit}
            total={query.data.total}
            hasMore={query.data.has_more}
            onPageChange={setOffset}
          />
          <div className="space-y-8">
            {(query.data.data ?? []).map((item) => (
              <EmpresaAggregateCard key={item.cnpj_basico} data={item} />
            ))}
          </div>
          <ExportPanel
            filters={{ cnpj_basico: cnpjBasico, razao_social: debouncedRazao }}
            columns={['cnpj_basico', 'razao_social', 'porte_empresa', 'capital_social']}
          />
        </>
      )}

      {query.data?.data?.[0] && (
        <p className="text-sm text-slate-500">
          Tip: open branches via{' '}
          <Link className="text-brand-500 hover:underline" to={`/estabelecimentos?cnpj_basico=${query.data.data[0].cnpj_basico}`}>
            Estabelecimento search
          </Link>
        </p>
      )}
    </div>
  )
}
