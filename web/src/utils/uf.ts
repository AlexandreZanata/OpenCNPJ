import type { LookupItem } from '../api/types'

const UF_CODE = /^[A-Za-z]{2}$/
const UF_LABEL_PREFIX = /^([A-Za-z]{2})\s*—/

/** Resolve UF from combobox selection or typed query (e.g. "PR" or "PR — Paraná"). */
export function resolveBrazilianUF(selected: LookupItem | null, query: string): string | undefined {
  if (selected?.code) {
    return selected.code.toUpperCase()
  }
  const trimmed = query.trim()
  if (UF_CODE.test(trimmed)) {
    return trimmed.toUpperCase()
  }
  const fromLabel = trimmed.match(UF_LABEL_PREFIX)
  if (fromLabel) {
    return fromLabel[1].toUpperCase()
  }
  return undefined
}
