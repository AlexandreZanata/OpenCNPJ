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
import { ProgressBar } from '../components/ui/ProgressBar'
import { resolveBrazilianUF } from '../utils/uf'

type SectorSelection = { type: 'preset' | 'cnae'; code: string; label: string } | null

export function PhoneExportPage() {
  const [sector, setSector] = useState<SectorSelection>(null)
  const [uf, setUf] = useState<LookupItem | null>(null)
  const [ufQuery, setUfQuery] = useState('')
  const [city, setCity] = useState<LookupItem | null>(null)
  const [cityQuery, setCityQuery] = useState('')
  const [nomeFantasia, setNomeFantasia] = useState<LookupItem | null>(null)
  const [createdFrom, setCreatedFrom] = useState('')
  const [createdTo, setCreatedTo] = useState('')
  const [exportAll, setExportAll] = useState(false)
  const [limit, setLimit] = useState(5000)
  const [onlyActive, setOnlyActive] = useState(true)
  const [loading, setLoading] = useState<'csv' | 'txt' | null>(null)
  const [progress, setProgress] = useState<number | null>(null)
  const [error, setError] = useState('')

  const resolvedUf = useMemo(() => resolveBrazilianUF(uf, ufQuery), [uf, ufQuery])

  const querySectors = useCallback((q: string) => lookupSectors(q, 20), [])
  const queryUF = useCallback((q: string) => lookupUF(q), [])
  const queryCity = useCallback(
    (q: string) => lookupMunicipio(q, resolvedUf ?? '', 20),
    [resolvedUf],
  )
  const queryNome = useCallback(
    (q: string) => lookupNomeFantasia(q, resolvedUf ?? '', 15),
    [resolvedUf],
  )

  const resolvedCityCode = city?.code
  const resolvedCityName = city?.code
    ? undefined
    : (cityQuery.trim().length >= 2 ? cityQuery.trim() : undefined)

  const payload = useMemo<PhoneExportRequest>(() => ({
    category: sector?.type === 'preset' ? sector.code : '',
    cnae: sector?.type === 'cnae' ? sector.code : undefined,
    uf: resolvedUf ?? city?.uf ?? undefined,
    municipio: resolvedCityCode || undefined,
    municipio_nome: resolvedCityName,
    nome_fantasia: nomeFantasia?.code || undefined,
    created_from: createdFrom || undefined,
    created_to: createdTo || undefined,
    only_active: onlyActive,
    export_all: exportAll,
    limit: exportAll ? undefined : limit,
    format: 'csv',
  }), [sector, resolvedUf, resolvedCityCode, resolvedCityName, nomeFantasia, createdFrom, createdTo, onlyActive, exportAll, limit])

  const canExport = Boolean(
    sector?.code ||
    payload.cnae ||
    payload.nome_fantasia ||
    resolvedUf ||
    city?.uf ||
    resolvedCityCode ||
    resolvedCityName,
  )

  const runExport = async (format: 'csv' | 'txt') => {
    if (!canExport) {
      setError('Select at least one filter: category/CNAE, UF, city, or nome fantasia.')
      return
    }
    setLoading(format)
    setProgress(null)
    setError('')
    try {
      await exportPhones({ ...payload, format }, setProgress)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Export failed')
    } finally {
      setLoading(null)
      setProgress(null)
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Phone Export</h1>
        <p className="text-slate-400">
          Export phone contacts by UF, city, CNAE category, or business name. CNAE is optional.
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
            value={uf?.label ?? ufQuery}
            minChars={0}
            onQuery={queryUF}
            onInputChange={setUfQuery}
            onSelect={(item) => {
              setUf(item)
              setUfQuery(item?.label ?? '')
              setCity(null)
              setCityQuery('')
            }}
          />
          <SearchCombobox
            label="City (Município)"
            placeholder="Curitiba, Campinas…"
            value={city?.label ?? cityQuery}
            hint="Type to search; pick from list for exact IBGE code (works with or without UF)"
            minChars={2}
            onQuery={queryCity}
            onInputChange={setCityQuery}
            onSelect={(item) => {
              setCity(item)
              setCityQuery(item?.label ?? '')
            }}
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
            label="Activity start date (from)"
            type="date"
            value={createdFrom}
            onChange={(e) => setCreatedFrom(e.target.value)}
            hint="Receita Federal registration date (data_inicio_atividade)"
          />
          <Input
            label="Activity start date (to)"
            type="date"
            value={createdTo}
            onChange={(e) => setCreatedTo(e.target.value)}
          />
          <div className="md:col-span-2">
            <div className="flex flex-wrap items-end gap-4">
              <div className="min-w-[160px] flex-1">
                <Input
                  label="Max rows"
                  type="number"
                  min={100}
                  max={50000}
                  value={exportAll ? '' : limit}
                  disabled={exportAll}
                  placeholder={exportAll ? 'No limit' : undefined}
                  onChange={(e) => setLimit(Number(e.target.value) || 5000)}
                />
              </div>
              <label className="mb-2 flex items-center gap-2 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={exportAll}
                  onChange={(e) => setExportAll(e.target.checked)}
                />
                Export all matching rows
              </label>
            </div>
          </div>
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
        {loading && (
          <div className="mb-4">
            <ProgressBar
              percent={progress}
              label={progress === null ? 'Generating export…' : 'Downloading export…'}
            />
          </div>
        )}
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
