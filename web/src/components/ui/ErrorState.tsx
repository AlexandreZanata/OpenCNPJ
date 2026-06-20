import { AlertCircle } from 'lucide-react'

interface ErrorStateProps {
  title?: string
  message: string
}

export function ErrorState({ title = 'Request failed', message }: ErrorStateProps) {
  return (
    <div className="flex flex-col items-center gap-3 rounded-xl border border-red-900/50 bg-red-950/30 px-6 py-10 text-center">
      <AlertCircle className="h-8 w-8 text-red-400" />
      <div>
        <p className="font-semibold text-red-200">{title}</p>
        <p className="mt-1 text-sm text-red-300/80">{message}</p>
      </div>
    </div>
  )
}
