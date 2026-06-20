import { apiPostBlob } from './client'
import type { ExportRequest } from './types'

export async function exportCsv(request: ExportRequest): Promise<void> {
  const blob = await apiPostBlob('/export/csv', request)
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = `export-${Date.now()}.csv`
  anchor.click()
  URL.revokeObjectURL(url)
}
