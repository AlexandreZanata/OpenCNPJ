import { apiGet, apiPostBlob } from './client'
import type { ExportCategory, ExportRequest, PhoneExportRequest } from './types'

export async function downloadBlob(blob: Blob, filename: string): Promise<void> {
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename
  anchor.click()
  URL.revokeObjectURL(url)
}

export async function exportCsv(request: ExportRequest): Promise<void> {
  const blob = await apiPostBlob('/export/csv', request)
  await downloadBlob(blob, `export-${Date.now()}.csv`)
}

export async function exportPhones(request: PhoneExportRequest): Promise<void> {
  const blob = await apiPostBlob('/export/phones', request)
  const ext = request.format === 'txt' ? 'txt' : 'csv'
  const label = request.category || 'phones'
  await downloadBlob(blob, `${label}-${Date.now()}.${ext}`)
}

export function getExportCategories(): Promise<ExportCategory[]> {
  return apiGet<ExportCategory[]>('/export/categories')
}

export type { PhoneExportRequest, ExportCategory }
