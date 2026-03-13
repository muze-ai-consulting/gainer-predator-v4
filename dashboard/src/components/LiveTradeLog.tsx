import { useEffect, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { cn } from '@/lib/utils'
import type { BotEvent } from '@/lib/api'

interface LiveTradeLogProps {
  events: BotEvent[]
}

interface BadgeConfig {
  label: string
  className: string
}

function getBadge(eventType: string): BadgeConfig {
  switch (eventType) {
    case 'trade_opened':
      return { label: 'OPEN', className: 'bg-[#00ff88]/10 text-[#00ff88] border-[#00ff88]/20' }
    case 'trade_closed':
      return { label: 'CLOSE', className: 'bg-[#ff4444]/10 text-[#ff4444] border-[#ff4444]/20' }
    case 'scan_result':
      return { label: 'SCAN', className: 'bg-[#3b82f6]/10 text-[#3b82f6] border-[#3b82f6]/20' }
    case 'signal_detected':
      return { label: 'SIGNAL', className: 'bg-[#ffcc00]/10 text-[#ffcc00] border-[#ffcc00]/20' }
    default:
      return { label: eventType.toUpperCase(), className: 'bg-[#888]/10 text-[#888] border-[#888]/20' }
  }
}

function formatTimestamp(ts: string): string {
  const d = new Date(ts)
  return d.toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function formatEventData(event: BotEvent): string {
  const d = event.data

  switch (event.event_type) {
    case 'trade_opened':
      return `${d.symbol ?? '???'} ${String(d.side ?? '').toUpperCase()}`
    case 'trade_closed': {
      const pnl = typeof d.pnl === 'number' ? (d.pnl >= 0 ? `+${d.pnl.toFixed(2)}` : d.pnl.toFixed(2)) : '?'
      return `${d.symbol ?? '???'} PnL: ${pnl}% | ${d.exit_reason ?? ''}`
    }
    case 'scan_result':
      return `${d.candidates ?? 0} candidates | hour ${d.hour ?? '?'}`
    case 'signal_detected':
      return `${d.symbol ?? '???'} rvol: ${d.rvol ?? '?'} jump: ${d.jump ?? '?'}`
    default:
      return JSON.stringify(d)
  }
}

function getCloseBadge(event: BotEvent): BadgeConfig {
  if (event.event_type !== 'trade_closed') return getBadge(event.event_type)
  const pnl = typeof event.data.pnl === 'number' ? event.data.pnl : 0
  return pnl >= 0
    ? { label: 'CLOSE', className: 'bg-[#00ff88]/10 text-[#00ff88] border-[#00ff88]/20' }
    : { label: 'CLOSE', className: 'bg-[#ff4444]/10 text-[#ff4444] border-[#ff4444]/20' }
}

export function LiveTradeLog({ events }: LiveTradeLogProps) {
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [events.length])

  return (
    <div className="bg-[#111111] border border-[#222] rounded-xl overflow-hidden">
      <div className="px-4 py-3 border-b border-[#222]">
        <h2 className="text-xs font-mono uppercase tracking-wider text-[#888]">
          Live Trade Log
        </h2>
      </div>

      <div
        ref={scrollRef}
        className="max-h-[400px] overflow-y-auto p-2 space-y-1 scrollbar-thin scrollbar-thumb-[#333] scrollbar-track-transparent"
      >
        {events.length === 0 && (
          <p className="text-[#888] text-xs font-mono text-center py-8">
            Waiting for events...
          </p>
        )}

        <AnimatePresence initial={false}>
          {events.map((event, i) => {
            const badge = event.event_type === 'trade_closed'
              ? getCloseBadge(event)
              : getBadge(event.event_type)

            return (
              <motion.div
                key={`${event.timestamp}-${i}`}
                className="flex items-center gap-3 px-3 py-1.5 rounded-lg hover:bg-[#1a1a1a] transition-colors"
                initial={{ opacity: 0, x: -12 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 12 }}
                transition={{ duration: 0.15 }}
              >
                <span className="text-[#555] text-xs font-mono shrink-0">
                  {formatTimestamp(event.timestamp)}
                </span>

                <span
                  className={cn(
                    'text-[10px] font-mono font-semibold px-1.5 py-0.5 rounded border shrink-0',
                    badge.className
                  )}
                >
                  {badge.label}
                </span>

                <span className="text-[#fafafa] text-xs font-mono truncate">
                  {formatEventData(event)}
                </span>
              </motion.div>
            )
          })}
        </AnimatePresence>
      </div>
    </div>
  )
}
