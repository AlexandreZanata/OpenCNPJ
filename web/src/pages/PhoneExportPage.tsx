import { Download, FileText } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { exportPhones } from '../api/exportApi'
import {
  lookupMunicipio,
  lookupNomeFantasia,
  lookupSectors,
  lookupUF,
} from '../api/lookupApi'
import type { LookupItem, PhoneExportRequest } from '../api/types'
import { SearchCombobox } from '../components/search/SearchCombobox'
import { Button } from '../components/ui/Button'
import { Card } from '../components/ui/Card'
import { Input } from '../components/ui/Input'

type SectorSelection = { type: 'preset' | 'cnae'; code: string; label: string } | null

export function PhoneExportPage() {
  const [sector, setSector] = useState<SectorSelection>(null)
  const [uf, setUf] = useState<LookupItem | null>(null)
  const [city, setCity] = useState<LookupItem | null>(null)
  const [nomeFantasia, setNomeFantasia] = useState<LookupItem | null>(null)
  const [limit, setLimit] = useState(5000)
  const [onlyActive, setOnlyActive] = useState(true)
  const [loading, setLoading] = useState<'csv' | 'txt' | null>(null)
  const [error, setError] = useState('')

  const querySectors = useCallback((q: string) => lookupSectors(q, 20), [])
  const queryUF = useCallback((q: string) => lookupUF(q), [])
  const queryCity = useCallback(
    (q: string) => lookupMunicipio(q, uf?.code ?? '', 20),
    [uf?.code],
  )
  const queryNome = useCallback(
    (q: string) => lookupNomeFantasia(q, uf?.code ?? '', 15),
    [uf?.code],
  )

  const payload = useMemo<PhoneExportRequest>(() => {
    const body: PhoneExportRequest = {
      category: sector?.type === 'preset' ? sector.code : '',
      cnae: sector?.type === 'cnae' ? sector.code : undefined,
      uf: uf?.code || undefined,
      municipio: city?.code || undefined,
      nome_fantasia: nomeFantasia?.code || undefined,
      only_active: onlyActive,
      limit,
      format: 'csv',
    }
    return body
  }, [sector, uf, city, nomeFantasia, onlyActive, limit])

  const canExport = Boolean(
    sector?.code || payload.cnae || payload.nome_fantasia,
  )

  const runExport = async (format: 'csv' | 'txt') => {
    if (!canExport) {
      setError('Select a category/CNAE or nome fantasia filter before exporting.')
      return
    }
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
          Smart search by CNAE code or description. Type to find categories, cities, and business names instantly.
        </p>
      </div>

      <Card title="Smart Filters">
        <div className="grid gap-4 md:grid-cols-2">
          <SearchCombobox
            label="Category / CNAE"
            placeholder="advocacia, 6911701, restaurante…"
            value={sector?.label ?? ''}
            hint="Search presets or CNAE catalog by code or description"
            minChars={0}
            onQuery={querySectors}
            onSelect={(item) => {
              if (!item) {
                setSector(null)
                return
              }
              setSector({
                type: item.type === 'preset' ? 'preset' : 'cnae',
                code: item.code,
                label: item.label,
              })
            }}
          />
          <SearchCombobox
            label="UF"
            placeholder="SP, São Paulo, Paraná…"
            value={uf?.label ?? ''}
            minChars={0}
            onQuery={queryUF}
            onSelect={(item) => {
              setUf(item)
              setCity(null)
            }}
          />
          <SearchCombobox
            label="City (Município)"
            placeholder="Curitiba, Campinas…"
            value={city?.label ?? ''}
            hint="Uses IBGE municipality code when selected — faster export"
            minChars={2}
            onQuery={queryCity}
            onSelect={setCity}
          />
          <SearchCombobox
            label="Nome Fantasia"
            placeholder="Type at least 3 characters…"
            value={nomeFantasia?.label ?? ''}
            hint="Suggestions from active establishments"
            minChars={3}
            onQuery={queryNome}
            onSelect={setNomeFantasia}
          />
          <Input
            label="Max rows"
            type="number"
            min={100}
            max={50000}
            value={limit}
            onChange={(e) => setLimit(Number(e.target.value) || 5000)}
          />
        </div>
        <label className="mt-4 flex items-center gap-2 text-sm text-slate-300">
          <input type="checkbox" checked={onlyActive} onChange={(e) => setOnlyActive(e.target.checked)} />
          Active establishments only
        </label>
      </Card>

      <Card title="Download">
        <p className="mb-4 text-sm text-slate-400">
          CSV: full aggregated data. TXT: one phone per line for dialers.
        </p>
        <div className="flex flex-wrap gap-3">
          <Button onClick={() => runExport('csv')} disabled={loading !== null || !canExport}>
            <Download className="h-4 w-4" />
            {loading === 'csv' ? 'Exporting CSV…' : 'Export CSV'}
          </Button>
          <Button variant="secondary" onClick={() => runExport('txt')} disabled={loading !== null || !canExport}>
            <FileText className="h-4 w-4" />
            {loading === 'txt' ? 'Exporting TXT…' : 'Export TXT'}
          </Button>
        </div>
        {error && <p className="mt-3 text-sm text-red-400">{error}</p>}
      </Card>
    </div>
  )
}
