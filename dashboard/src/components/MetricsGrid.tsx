import { motion, AnimatePresence } from 'framer-motion'
import { cn, formatUsd, formatPnl } from '@/lib/utils'

interface Metrics {
  balance?: number
  active_positions?: number
  total_trades?: number
  win_rate?: number
  net_pnl?: number
  profit_factor?: number
  max_drawdown?: number
  avg_pnl?: number
  score?: number
}

interface MetricCardProps {
  label: string
  value: string
  color?: string
}

function MetricCard({ label, value, color }: MetricCardProps) {
  return (
    <div className="bg-[#111111] border border-[#222] rounded-xl p-4">
      <p className="text-[#888] text-xs uppercase tracking-wider font-mono mb-2">
        {label}
      </p>
      <AnimatePresence mode="wait">
        <motion.p
          key={value}
          className={cn('text-2xl font-bold font-mono', color ?? 'text-[#fafafa]')}
          initial={{ opacity: 0, y: 4 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -4 }}
          transition={{ duration: 0.2 }}
        >
          {value}
        </motion.p>
      </AnimatePresence>
    </div>
  )
}

function pnlColor(value: number): string {
  if (value > 0) return 'text-[#00ff88]'
  if (value < 0) return 'text-[#ff4444]'
  return 'text-[#fafafa]'
}

function winRateColor(rate: number): string {
  if (rate >= 60) return 'text-[#00ff88]'
  if (rate >= 45) return 'text-[#ffcc00]'
  return 'text-[#ff4444]'
}

export function MetricsGrid({ metrics }: { metrics: Metrics | null }) {
  if (!metrics) {
    return (
      <div className="grid grid-cols-3 gap-5">
        {Array.from({ length: 9 }).map((_, i) => (
          <div
            key={i}
            className="bg-[#111111] border border-[#222] rounded-xl p-4 animate-pulse h-24"
          />
        ))}
      </div>
    )
  }

  const bal = metrics.balance ?? 0
  const positions = metrics.active_positions ?? 0
  const trades = metrics.total_trades ?? 0
  const wr = metrics.win_rate ?? 0
  const pnl = metrics.net_pnl ?? 0
  const pf = metrics.profit_factor ?? 0
  const dd = metrics.max_drawdown ?? 0
  const avg = metrics.avg_pnl ?? 0
  const sc = metrics.score ?? 0

  const cards: MetricCardProps[] = [
    { label: 'Balance', value: formatUsd(bal) },
    { label: 'Active Positions', value: String(positions) },
    { label: 'Total Trades', value: String(trades) },
    { label: 'Win Rate', value: `${wr.toFixed(1)}%`, color: winRateColor(wr) },
    { label: 'Net PnL', value: formatUsd(pnl), color: pnlColor(pnl) },
    { label: 'Profit Factor', value: pf.toFixed(2) },
    { label: 'Max Drawdown', value: formatPnl(-Math.abs(dd)), color: 'text-[#ff4444]' },
    { label: 'Avg PnL', value: formatUsd(avg), color: pnlColor(avg) },
    { label: 'Score', value: sc.toFixed(2) },
  ]

  return (
    <div className="grid grid-cols-3 gap-5">
      {cards.map((card) => (
        <MetricCard key={card.label} {...card} />
      ))}
    </div>
  )
}
