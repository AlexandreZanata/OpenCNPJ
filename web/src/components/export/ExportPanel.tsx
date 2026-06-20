import { Download } from 'lucide-react'
import { useState } from 'react'
import { exportCsv } from '../../api/exportApi'
import { Button } from '../ui/Button'
import { Card } from '../ui/Card'

interface ExportPanelProps {
  filters: Record<string, string | number>
  columns: string[]
}

export function ExportPanel({ filters, columns }: ExportPanelProps) {
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const handleExport = async () => {
    setLoading(true)
    setError('')
    try {
      await exportCsv({ filters: { ...filters, limit: 1000 }, selected_columns: columns, format: 'csv' })
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Export failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <Card title="Export CSV">
      <p className="mb-4 text-sm text-slate-400">Download up to 1,000 rows matching current filters.</p>
      <Button onClick={handleExport} disabled={loading}>
        <Download className="h-4 w-4" />
        {loading ? 'Exporting…' : 'Export CSV'}
      </Button>
      {error && <p className="mt-2 text-sm text-red-400">{error}</p>}
    </Card>
  )
}
