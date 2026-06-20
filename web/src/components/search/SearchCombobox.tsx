import { useEffect, useId, useRef, useState } from 'react'
import type { LookupItem } from '../../api/types'
import { useDebounce } from '../../hooks/useDebounce'

interface SearchComboboxProps {
  label: string
  placeholder: string
  value: string
  hint?: string
  minChars?: number
  onQuery: (query: string) => Promise<LookupItem[]>
  onSelect: (item: LookupItem | null) => void
  onInputChange?: (text: string) => void
}

export function SearchCombobox({
  label,
  placeholder,
  value,
  hint,
  minChars = 0,
  onQuery,
  onSelect,
  onInputChange,
}: SearchComboboxProps) {
  const listId = useId()
  const rootRef = useRef<HTMLDivElement>(null)
  const [input, setInput] = useState(value)
  const [open, setOpen] = useState(false)
  const [loading, setLoading] = useState(false)
  const [items, setItems] = useState<LookupItem[]>([])
  const debounced = useDebounce(input, 250)

  useEffect(() => {
    setInput(value)
  }, [value])

  useEffect(() => {
    if (!open) {
      return
    }
    if (debounced.length < minChars) {
      setItems([])
      return
    }

    let cancelled = false
    setLoading(true)
    onQuery(debounced)
      .then((results) => {
        if (!cancelled) {
          setItems(results)
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [debounced, minChars, onQuery, open])

  useEffect(() => {
    const onClick = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', onClick)
    return () => document.removeEventListener('mousedown', onClick)
  }, [])

  return (
    <div ref={rootRef} className="relative flex flex-col gap-1.5 text-sm">
      <label className="font-medium text-slate-300">{label}</label>
      <input
        className="rounded-lg border border-border bg-slate-900 px-3 py-2 text-slate-100 outline-none focus:border-brand-500"
        placeholder={placeholder}
        value={input}
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        onFocus={() => setOpen(true)}
        onChange={(event) => {
          setInput(event.target.value)
          onInputChange?.(event.target.value)
          onSelect(null)
          setOpen(true)
        }}
      />
      {hint && <span className="text-xs text-slate-500">{hint}</span>}
      {open && (
        <ul
          id={listId}
          className="absolute top-full z-20 mt-1 max-h-56 w-full overflow-y-auto rounded-lg border border-border bg-slate-900 shadow-xl"
        >
          {loading && <li className="px-3 py-2 text-slate-500">Searching…</li>}
          {!loading && items.length === 0 && debounced.length >= minChars && (
            <li className="px-3 py-2 text-slate-500">No matches</li>
          )}
          {!loading &&
            items.map((item) => (
              <li key={`${item.type}-${item.code}`}>
                <button
                  type="button"
                  className="w-full px-3 py-2 text-left hover:bg-slate-800"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => {
                    setInput(item.label)
                    onSelect(item)
                    setOpen(false)
                  }}
                >
                  <span className="block text-slate-100">{item.label}</span>
                  {item.description && (
                    <span className="block text-xs text-slate-500">{item.description}</span>
                  )}
                </button>
              </li>
            ))}
        </ul>
      )}
    </div>
  )
}
