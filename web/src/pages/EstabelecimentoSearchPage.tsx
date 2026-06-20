import { useQuery } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { searchEstabelecimentos } from '../api/estabelecimentoApi'
import type { Estabelecimento } from '../api/types'
import { ExportPanel } from '../components/export/ExportPanel'
import { Card } from '../components/ui/Card'
import { DataTable, type Column } from '../components/ui/DataTable'
import { ErrorState } from '../components/ui/ErrorState'
import { Input } from '../components/ui/Input'
import { LoadingState } from '../components/ui/LoadingState'
import { Pagination } from '../components/ui/Pagination'
import { useDebounce } from '../hooks/useDebounce'
import { formatCnpj } from '../utils/cnpj'
import { unwrapString } from '../utils/format'

const PAGE_SIZE = 20
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

  const columns: Column<Estabelecimento>[] = [
    {
      key: 'cnpj',
      header: 'CNPJ',
      render: (row) => (
        <Link className="font-mono text-brand-400 hover:underline" to={`/cnpj/${row.cnpj_completo}`}>
          {formatCnpj(row.cnpj_completo)}
        </Link>
      ),
    },
    { key: 'razao', header: 'Razão Social', render: (row) => unwrapString(row.razao_social) },
    { key: 'fantasia', header: 'Nome Fantasia', render: (row) => unwrapString(row.nome_fantasia) || '—' },
    { key: 'uf', header: 'UF', render: (row) => unwrapString(row.uf) },
    { key: 'cnae', header: 'CNAE', render: (row) => unwrapString(row.cnae_fiscal_principal) },
  ]

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Estabelecimento Search</h1>
        <p className="text-slate-400">Search branches by CNPJ, name, UF, or CNAE. Combine filters to narrow results.</p>
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
          <Card title="Results">
            <DataTable columns={columns} rows={query.data.data} />
            <Pagination
              offset={query.data.offset}
              limit={query.data.limit}
              total={query.data.total}
              hasMore={query.data.has_more}
              onPageChange={setOffset}
            />
          </Card>
          <ExportPanel
            filters={{ uf, cnae, nome_fantasia: debouncedNome, cnpj_basico: cnpjBasico }}
            columns={['cnpj_completo', 'nome_fantasia', 'razao_social', 'uf', 'cnae_fiscal_principal']}
          />
        </>
      )}
    </div>
  )
}
