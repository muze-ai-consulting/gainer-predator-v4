import { useMemo } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { usePolling } from '@/hooks/usePolling'
import { fetchStatus } from '@/lib/api'
import { cn, formatUsd } from '@/lib/utils'

interface Position {
  symbol: string
  side: string
  entry_price: number
  unrealized_pnl: number
  hold_time: string
}

interface StatusResponse {
  positions: Position[]
}

function formatHoldTime(ht: string): string {
  return ht
}

function pnlColor(value: number): string {
  if (value > 0) return 'text-[#00ff88]'
  if (value < 0) return 'text-[#ff4444]'
  return 'text-[#fafafa]'
}

function borderGlow(value: number): string {
  if (value > 0) return 'rgba(0,255,136,0.3)'
  if (value < 0) return 'rgba(255,68,68,0.3)'
  return 'rgba(34,34,34,1)'
}

export function ActivePositions() {
  const fetcher = useMemo(() => fetchStatus, [])
  const { data } = usePolling<StatusResponse>(fetcher, 3000)

  const positions = data?.positions ?? []

  return (
    <div>
      <div className="mb-4">
        <h2 className="text-xs font-mono uppercase tracking-wider text-[#888]">
          Active Positions
        </h2>
      </div>

      {positions.length === 0 ? (
        <div className="bg-[#111111] border border-[#222] rounded-xl p-8 text-center">
          <p className="text-[#888] text-sm font-mono">No active positions</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <AnimatePresence mode="popLayout">
            {positions.map((pos) => (
              <motion.div
                key={`${pos.symbol}-${pos.side}`}
                className="bg-[#111111] border border-[#222] rounded-xl p-4"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{
                  opacity: 1,
                  scale: 1,
                  boxShadow: [
                    `0 0 0px ${borderGlow(pos.unrealized_pnl)}`,
                    `0 0 12px ${borderGlow(pos.unrealized_pnl)}`,
                    `0 0 0px ${borderGlow(pos.unrealized_pnl)}`,
                  ],
                }}
                exit={{ opacity: 0, scale: 0.95 }}
                transition={{
                  default: { duration: 0.2 },
                  boxShadow: { duration: 2, repeat: Infinity, ease: 'easeInOut' },
                }}
              >
                <div className="flex items-center justify-between mb-3">
                  <span className="text-[#fafafa] text-sm font-mono font-bold">
                    {pos.symbol}
                  </span>
                  <span
                    className={cn(
                      'text-[10px] font-mono font-semibold px-1.5 py-0.5 rounded border',
                      pos.side.toUpperCase() === 'LONG'
                        ? 'bg-[#00ff88]/10 text-[#00ff88] border-[#00ff88]/20'
                        : 'bg-[#ff4444]/10 text-[#ff4444] border-[#ff4444]/20'
                    )}
                  >
                    {pos.side.toUpperCase()}
                  </span>
                </div>

                <div className="space-y-2">
                  <div className="flex justify-between">
                    <span className="text-[#888] text-xs font-mono">Entry</span>
                    <span className="text-[#fafafa] text-xs font-mono">
                      {formatUsd(pos.entry_price)}
                    </span>
                  </div>

                  <div className="flex justify-between">
                    <span className="text-[#888] text-xs font-mono">uPnL</span>
                    <span className={cn('text-xs font-mono font-semibold', pnlColor(pos.unrealized_pnl))}>
                      {pos.unrealized_pnl >= 0 ? '+' : ''}
                      {formatUsd(pos.unrealized_pnl)}
                    </span>
                  </div>

                  <div className="flex justify-between">
                    <span className="text-[#888] text-xs font-mono">Hold Time</span>
                    <span className="text-[#fafafa] text-xs font-mono">
                      {formatHoldTime(pos.hold_time)}
                    </span>
                  </div>
                </div>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}
    </div>
  )
}
