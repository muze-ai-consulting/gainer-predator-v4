import { useCallback } from 'react'
import { useSSE } from '@/hooks/useSSE'
import { usePolling } from '@/hooks/usePolling'
import { fetchMetrics } from '@/lib/api'
import { Header } from '@/components/Header'
import { MetricsGrid } from '@/components/MetricsGrid'
import { LiveTradeLog } from '@/components/LiveTradeLog'
import { ActivePositions } from '@/components/ActivePositions'
import { EquityCurve } from '@/components/EquityCurve'
import { TradesTable } from '@/components/TradesTable'
import { HourHeatmap } from '@/components/HourHeatmap'
import { ExperimentPanel } from '@/components/ExperimentPanel'

export default function App() {
  const { events, connected } = useSSE()

  const metricsFetcher = useCallback(() => fetchMetrics(), [])
  const { data: metrics } = usePolling(metricsFetcher, 5000)

  return (
    <div className="min-h-screen bg-background px-6 py-8 md:px-10 md:py-10 lg:px-16 lg:py-12">
      <div className="mx-auto max-w-[1400px] space-y-8">
        <Header connected={connected} />

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
          <div className="lg:col-span-1">
            <LiveTradeLog events={events} />
          </div>

          <div className="lg:col-span-2 space-y-8">
            <MetricsGrid metrics={metrics} />
            <ActivePositions />
          </div>
        </div>

        <EquityCurve />

        <TradesTable />

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
          <HourHeatmap />
          <ExperimentPanel />
        </div>
      </div>
    </div>
  )
}
