export function serializeRecordJson(data: unknown): string {
  return JSON.stringify(data, null, 2)
}

export function downloadJson(filename: string, data: unknown): void {
  const blob = new Blob([serializeRecordJson(data)], { type: 'application/json;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename.endsWith('.json') ? filename : `${filename}.json`
  anchor.click()
  URL.revokeObjectURL(url)
}

export async function copyRecordToClipboard(data: unknown): Promise<void> {
  await navigator.clipboard.writeText(serializeRecordJson(data))
}
