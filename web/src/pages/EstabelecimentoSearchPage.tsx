import { useQuery } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { searchEstabelecimentos } from '../api/estabelecimentoApi'
import { EmpresaFullPanel, SimplesPanel, SocioList } from '../components/detail/EmpresaAggregateCard'
import { EstabelecimentoFullPanel } from '../components/detail/EstabelecimentoFullPanel'
import { RecordActions } from '../components/detail/RecordActions'
import { ExportPanel } from '../components/export/ExportPanel'
import { Card } from '../components/ui/Card'
import { ErrorState } from '../components/ui/ErrorState'
import { Input } from '../components/ui/Input'
import { LoadingState } from '../components/ui/LoadingState'
import { Pagination } from '../components/ui/Pagination'
import { useDebounce } from '../hooks/useDebounce'

const PAGE_SIZE = 5
const UF_OPTIONS = ['', 'AC', 'AL', 'AM', 'AP', 'BA', 'CE', 'DF', 'ES', 'GO', 'MA', 'MG', 'MS', 'MT', 'PA', 'PB', 'PE', 'PI', 'PR', 'RJ', 'RN', 'RO', 'RR', 'RS', 'SC', 'SE', 'SP', 'TO']

export function EstabelecimentoSearchPage() {
  const [searchParams] = useSearchParams()
  const [cnpj, setCnpj] = useState('')
  const [cnpjBasico, setCnpjBasico] = useState(searchParams.get('cnpj_basico') ?? '')
  const [nomeFantasia, setNomeFantasia] = useState('')
  const [uf, setUf] = useState('')
  const [cnae, setCnae] = useState('')
  const [offset, setOffset] = useState(0)
  const debouncedNome = useDebounce(nomeFantasia, 500)

  const params = useMemo(
    () => ({
      cnpj: cnpj.replace(/\D/g, '') || undefined,
      cnpj_basico: cnpjBasico || undefined,
      nome_fantasia: debouncedNome || undefined,
      uf: uf || undefined,
      cnae: cnae || undefined,
      limit: PAGE_SIZE,
      offset,
    }),
    [cnpj, cnpjBasico, debouncedNome, uf, cnae, offset],
  )

  const hasFilter = Boolean(params.cnpj || params.cnpj_basico || params.nome_fantasia || params.uf || params.cnae)

  const query = useQuery({
    queryKey: ['estabelecimentos', params],
    queryFn: () => searchEstabelecimentos(params),
    enabled: hasFilter,
    staleTime: 120_000,
  })

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Estabelecimento Search</h1>
        <p className="text-slate-400">
          Full branch data plus parent empresa, sócios, and Simples/MEI for each result.
        </p>
      </div>

      <Card title="Filters">
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          <Input label="CNPJ Completo" value={cnpj} onChange={(e) => { setOffset(0); setCnpj(e.target.value) }} />
          <Input label="CNPJ Básico" value={cnpjBasico} onChange={(e) => { setOffset(0); setCnpjBasico(e.target.value.replace(/\D/g, '').slice(0, 8)) }} />
          <Input label="Nome Fantasia" value={nomeFantasia} onChange={(e) => { setOffset(0); setNomeFantasia(e.target.value) }} />
          <Input label="CNAE Principal" value={cnae} onChange={(e) => { setOffset(0); setCnae(e.target.value) }} />
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="font-medium text-slate-300">UF</span>
            <select
              className="rounded-lg border border-border bg-slate-900 px-3 py-2 text-slate-100"
              value={uf}
              onChange={(e) => { setOffset(0); setUf(e.target.value) }}
            >
              {UF_OPTIONS.map((code) => (
                <option key={code || 'all'} value={code}>{code || 'All states'}</option>
              ))}
            </select>
          </label>
        </div>
      </Card>

      {!hasFilter && <p className="text-slate-500">Apply at least one filter to search estabelecimentos.</p>}
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
              <div key={item.cnpj_completo} className="space-y-4 rounded-xl border border-border/80 bg-surface-muted/30 p-4">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <Link className="font-mono text-brand-400 hover:underline" to={`/cnpj/${item.cnpj_completo}`}>
                    Open CNPJ lookup
                  </Link>
                  <RecordActions data={item} filename={`estabelecimento-${item.cnpj_completo}`} />
                </div>
                <EstabelecimentoFullPanel data={item} />
                <Card title="Empresa (parent)">
                  <EmpresaFullPanel data={item.empresa} />
                </Card>
                <Card title={`Sócios (${item.socios.length})`}>
                  <SocioList socios={item.socios} />
                </Card>
                <Card title="Simples Nacional / MEI">
                  <SimplesPanel data={item.simples} />
                </Card>
              </div>
            ))}
          </div>
          <ExportPanel
            filters={{ uf, cnae, nome_fantasia: debouncedNome, cnpj_basico: cnpjBasico }}
            columns={['cnpj_completo', 'nome_fantasia', 'razao_social', 'uf', 'cnae_fiscal_principal']}
          />
        </>
      )}
    </div>
  )
}
