import type { ReactNode } from 'react'

interface CardProps {
  title?: string
  children: ReactNode
  action?: ReactNode
}

export function Card({ title, children, action }: CardProps) {
  return (
    <section className="rounded-xl border border-border bg-surface-muted/60 p-5 shadow-lg shadow-black/20">
      {(title || action) && (
        <header className="mb-4 flex items-center justify-between gap-3">
          {title && <h2 className="text-lg font-semibold text-white">{title}</h2>}
          {action}
        </header>
      )}
      {children}
    </section>
  )
}
