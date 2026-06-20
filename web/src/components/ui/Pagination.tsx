import { ChevronLeft, ChevronRight } from 'lucide-react'
import { Button } from './Button'
import { formatNumber } from '../../utils/format'

interface PaginationProps {
  offset: number
  limit: number
  total: number
  hasMore: boolean
  onPageChange: (offset: number) => void
}

export function Pagination({ offset, limit, total, hasMore, onPageChange }: PaginationProps) {
  const page = Math.floor(offset / limit) + 1
  const canPrev = offset > 0
  const canNext = hasMore

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 pt-4 text-sm text-slate-400">
      <span>
        Page {page} · showing {formatNumber(Math.min(limit, total - offset > 0 ? limit : 0))} rows
        {total > 0 && ` · total ≥ ${formatNumber(total)}`}
      </span>
      <div className="flex gap-2">
        <Button variant="secondary" disabled={!canPrev} onClick={() => onPageChange(Math.max(0, offset - limit))}>
          <ChevronLeft className="h-4 w-4" /> Previous
        </Button>
        <Button variant="secondary" disabled={!canNext} onClick={() => onPageChange(offset + limit)}>
          Next <ChevronRight className="h-4 w-4" />
        </Button>
      </div>
    </div>
  )
}
