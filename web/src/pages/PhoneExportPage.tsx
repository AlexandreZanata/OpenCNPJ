import { useQuery } from '@tanstack/react-query'
import { Download, FileText } from 'lucide-react'
import { useMemo, useState } from 'react'
import { exportPhones, getExportCategories } from '../api/exportApi'
import type { PhoneExportRequest } from '../api/types'
import { Button } from '../components/ui/Button'
import { Card } from '../components/ui/Card'
import { Input } from '../components/ui/Input'

const UF_OPTIONS = ['', 'AC', 'AL', 'AM', 'AP', 'BA', 'CE', 'DF', 'ES', 'GO', 'MA', 'MG', 'MS', 'MT', 'PA', 'PB', 'PE', 'PI', 'PR', 'RJ', 'RN', 'RO', 'RR', 'RS', 'SC', 'SE', 'SP', 'TO']

export function PhoneExportPage() {
  const categories = useQuery({ queryKey: ['export', 'categories'], queryFn: getExportCategories, staleTime: 600_000 })
  const [category, setCategory] = useState('advocacia')
  const [uf, setUf] = useState('')
  const [city, setCity] = useState('')
  const [cnae, setCnae] = useState('')
  const [nomeFantasia, setNomeFantasia] = useState('')
  const [limit, setLimit] = useState(5000)
  const [onlyActive, setOnlyActive] = useState(true)
  const [loading, setLoading] = useState<'csv' | 'txt' | null>(null)
  const [error, setError] = useState('')

  const payload = useMemo<PhoneExportRequest>(() => ({
    category,
    cnae: cnae || undefined,
    uf: uf || undefined,
    municipio_nome: city || undefined,
    nome_fantasia: nomeFantasia || undefined,
    only_active: onlyActive,
    limit,
    format: 'csv',
  }), [category, cnae, uf, city, nomeFantasia, onlyActive, limit])

  const selectedCategory = categories.data?.find((item) => item.key === category)

  const runExport = async (format: 'csv' | 'txt') => {
    setLoading(format)
    setError('')
    try {
      await exportPhones({ ...payload, format })
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Export failed')
    } finally {
      setLoading(null)
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Phone Export</h1>
        <p className="text-slate-400">
          Export establishment phone contacts by business category with city and UF filters.
          CSV includes full aggregated data; TXT is optimized for dialer lists.
        </p>
      </div>

      <Card title="Category & Filters">
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="font-medium text-slate-300">Category</span>
            <select
              className="rounded-lg border border-border bg-slate-900 px-3 py-2 text-slate-100"
              value={category}
              onChange={(e) => setCategory(e.target.value)}
            >
              {(categories.data ?? []).map((item) => (
                <option key={item.key} value={item.key}>{item.label}</option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="font-medium text-slate-300">UF</span>
            <select
              className="rounded-lg border border-border bg-slate-900 px-3 py-2 text-slate-100"
              value={uf}
              onChange={(e) => setUf(e.target.value)}
            >
              {UF_OPTIONS.map((code) => (
                <option key={code || 'all'} value={code}>{code || 'All states'}</option>
              ))}
            </select>
          </label>
          <Input label="City (Município name)" value={city} onChange={(e) => setCity(e.target.value)} placeholder="São Paulo, Curitiba…" />
          <Input label="CNAE override" value={cnae} onChange={(e) => setCnae(e.target.value.replace(/\D/g, '').slice(0, 7))} placeholder="6911701" />
          <Input label="Nome Fantasia contains" value={nomeFantasia} onChange={(e) => setNomeFantasia(e.target.value)} placeholder="Optional extra filter" />
          <Input label="Max rows" type="number" min={100} max={50000} value={limit} onChange={(e) => setLimit(Number(e.target.value) || 5000)} />
        </div>
        <label className="mt-4 flex items-center gap-2 text-sm text-slate-300">
          <input type="checkbox" checked={onlyActive} onChange={(e) => setOnlyActive(e.target.checked)} />
          Active establishments only (situacao_cadastral = 2)
        </label>
        {selectedCategory && (
          <p className="mt-3 text-sm text-slate-500">
            {selectedCategory.description} · CNAE: {selectedCategory.cnae_codes.join(', ')}
          </p>
        )}
      </Card>

      <Card title="Download">
        <p className="mb-4 text-sm text-slate-400">
          CSV: CNPJ, razão social, phones, email, city, UF, CNAE.
          TXT: one line per phone — <code className="text-brand-400">phone | name | city/UF | CNAE description</code>
        </p>
        <div className="flex flex-wrap gap-3">
          <Button onClick={() => runExport('csv')} disabled={loading !== null}>
            <Download className="h-4 w-4" />
            {loading === 'csv' ? 'Exporting CSV…' : 'Export CSV'}
          </Button>
          <Button variant="secondary" onClick={() => runExport('txt')} disabled={loading !== null}>
            <FileText className="h-4 w-4" />
            {loading === 'txt' ? 'Exporting TXT…' : 'Export TXT'}
          </Button>
        </div>
        {error && <p className="mt-3 text-sm text-red-400">{error}</p>}
      </Card>
    </div>
  )
}
