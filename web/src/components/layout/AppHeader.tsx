import { useQuery } from '@tanstack/react-query'
import { Activity } from 'lucide-react'
import { checkHealth } from '../../api/client'

export function AppHeader() {
  const health = useQuery({ queryKey: ['health'], queryFn: checkHealth, refetchInterval: 30_000 })

  return (
    <header className="flex items-center justify-between border-b border-border bg-surface/80 px-6 py-4 backdrop-blur">
      <div>
        <p className="text-sm text-slate-400">Federal open-data search platform</p>
      </div>
      <div className="flex items-center gap-2 text-sm">
        <Activity className={`h-4 w-4 ${health.data ? 'text-emerald-400' : 'text-red-400'}`} />
        <span className={health.data ? 'text-emerald-300' : 'text-red-300'}>
          API {health.isLoading ? 'checking…' : health.data ? 'ready' : 'unavailable'}
        </span>
      </div>
    </header>
  )
}
