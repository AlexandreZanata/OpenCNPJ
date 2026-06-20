import type { InputHTMLAttributes } from 'react'

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label: string
}

export function Input({ label, className = '', id, ...props }: InputProps) {
  const inputId = id ?? label.toLowerCase().replace(/\s+/g, '-')
  return (
    <label htmlFor={inputId} className="flex flex-col gap-1.5 text-sm">
      <span className="font-medium text-slate-300">{label}</span>
      <input
        id={inputId}
        className={`rounded-lg border border-border bg-slate-900 px-3 py-2 text-slate-100 outline-none focus:border-brand-500 ${className}`}
        {...props}
      />
    </label>
  )
}
