import type { ErrorResponse } from './types'

const API_BASE = import.meta.env.VITE_API_BASE_URL ?? '/api/v1'

export class ApiError extends Error {
  status: number
  code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}

function buildUrl(path: string, params?: Record<string, string | number | undefined>): string {
  const url = new URL(`${API_BASE}${path}`, window.location.origin)
  if (params) {
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined && value !== '') {
        url.searchParams.set(key, String(value))
      }
    })
  }
  return url.pathname + url.search
}

async function parseError(response: Response): Promise<ApiError> {
  try {
    const body = (await response.json()) as ErrorResponse
    return new ApiError(response.status, body.error, body.message ?? response.statusText)
  } catch {
    return new ApiError(response.status, 'unknown_error', response.statusText)
  }
}

export async function apiGet<T>(path: string, params?: Record<string, string | number | undefined>): Promise<T> {
  const url = buildUrl(path, params)
  const response = await fetch(url, { headers: { Accept: 'application/json' } })
  if (!response.ok) {
    throw await parseError(response)
  }
  return response.json() as Promise<T>
}

export type ProgressCallback = (percent: number | null) => void

async function readBlobWithProgress(
  response: Response,
  onProgress?: ProgressCallback,
): Promise<Blob> {
  const total = Number(response.headers.get('Content-Length')) || 0
  const body = response.body
  if (!body) {
    onProgress?.(100)
    return response.blob()
  }

  const reader = body.getReader()
  const chunks: Uint8Array[] = []
  let received = 0

  onProgress?.(total > 0 ? 0 : null)

  for (;;) {
    const { done, value } = await reader.read()
    if (done) {
      break
    }
    chunks.push(value)
    received += value.length
    if (total > 0) {
      onProgress?.(Math.min(99, Math.round((received / total) * 100)))
    }
  }

  onProgress?.(100)
  const type = response.headers.get('Content-Type') ?? 'application/octet-stream'
  return new Blob(chunks as BlobPart[], { type })
}

export async function apiPostBlob(path: string, body: unknown): Promise<Blob> {
  const url = buildUrl(path)
  const response = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'text/csv' },
    body: JSON.stringify(body),
  })
  if (!response.ok) {
    throw await parseError(response)
  }
  return response.blob()
}

export async function apiPostBlobWithProgress(
  path: string,
  body: unknown,
  onProgress?: ProgressCallback,
): Promise<Blob> {
  const url = buildUrl(path)
  const response = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'text/csv' },
    body: JSON.stringify(body),
  })
  if (!response.ok) {
    throw await parseError(response)
  }
  return readBlobWithProgress(response, onProgress)
}

export async function checkHealth(): Promise<boolean> {
  try {
    const response = await fetch('/readyz')
    return response.ok
  } catch {
    return false
  }
}
