type ProgressBarProps = {
  percent: number | null
  label?: string
}

export function ProgressBar({ percent, label }: ProgressBarProps) {
  const indeterminate = percent === null

  return (
    <div className="space-y-2" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent ?? undefined}>
      <div className="flex items-center justify-between text-xs text-slate-400">
        <span>{label ?? 'Preparing export…'}</span>
        {!indeterminate && percent !== null && <span>{percent}%</span>}
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-slate-800">
        {indeterminate ? (
          <div className="progress-indeterminate h-full w-1/3 rounded-full bg-emerald-500" />
        ) : (
          <div
            className="h-full rounded-full bg-emerald-500 transition-all duration-300"
            style={{ width: `${percent ?? 0}%` }}
          />
        )}
      </div>
    </div>
  )
}
