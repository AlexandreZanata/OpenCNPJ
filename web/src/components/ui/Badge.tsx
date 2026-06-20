interface BadgeProps {
  children: string
  tone?: 'success' | 'warning' | 'neutral'
}

const tones = {
  success: 'bg-emerald-950 text-emerald-300 ring-emerald-800',
  warning: 'bg-amber-950 text-amber-300 ring-amber-800',
  neutral: 'bg-slate-800 text-slate-300 ring-slate-700',
}

export function Badge({ children, tone = 'neutral' }: BadgeProps) {
  return (
    <span className={`inline-flex rounded-full px-2.5 py-0.5 text-xs font-medium ring-1 ring-inset ${tones[tone]}`}>
      {children}
    </span>
  )
}
