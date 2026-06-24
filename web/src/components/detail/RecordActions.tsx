import { Check, Copy, Download } from 'lucide-react'
import { useState } from 'react'
import { copyRecordToClipboard, downloadJson } from '../../utils/recordExport'
import { Button } from '../ui/Button'

interface RecordActionsProps {
  data: unknown
  filename: string
}

export function RecordActions({ data, filename }: RecordActionsProps) {
  const [copied, setCopied] = useState(false)
  const [error, setError] = useState('')

  const handleCopy = async () => {
    setError('')
    try {
      await copyRecordToClipboard(data)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 2000)
    } catch {
      setError('Copy failed')
    }
  }

  const handleExport = () => {
    setError('')
    try {
      downloadJson(filename, data)
    } catch {
      setError('Export failed')
    }
  }

  return (
    <div className="flex flex-col items-end gap-1">
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="secondary" className="px-3 py-1.5 text-xs" onClick={handleCopy}>
          {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
          {copied ? 'Copied' : 'Copy'}
        </Button>
        <Button type="button" variant="secondary" className="px-3 py-1.5 text-xs" onClick={handleExport}>
          <Download className="h-3.5 w-3.5" />
          Export
        </Button>
      </div>
      {error && <p className="text-xs text-red-400">{error}</p>}
    </div>
  )
}
