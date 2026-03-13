import { useMemo } from "react";
import {
  ResponsiveContainer,
  AreaChart,
  Area,
  XAxis,
  YAxis,
  Tooltip,
} from "recharts";
import { motion } from "framer-motion";
import { cn, formatPnl } from "@/lib/utils";
import { fetchTrades } from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";

interface Trade {
  id: string;
  symbol: string;
  side: "LONG" | "SHORT";
  entry_price: number;
  exit_price: number;
  pnl_pct: number;
  pnl_usd: number;
  exit_reason: string;
  open_time: string;
  close_time: string;
  leverage: number;
}

interface EquityPoint {
  time: string;
  equity: number;
}

export function EquityCurve() {
  const { data: trades } = usePolling<Trade[]>(fetchTrades, 10_000);

  const equityData = useMemo<EquityPoint[]>(() => {
    if (!trades || trades.length === 0) return [];

    const sorted = [...trades].sort(
      (a, b) =>
        new Date(a.close_time).getTime() - new Date(b.close_time).getTime()
    );

    let cumulative = 0;
    return sorted.map((t) => {
      cumulative += t.pnl_usd;
      return {
        time: new Date(t.close_time).toLocaleDateString("en-US", {
          month: "short",
          day: "numeric",
          hour: "2-digit",
          minute: "2-digit",
        }),
        equity: Number(cumulative.toFixed(2)),
      };
    });
  }, [trades]);

  const isPositive =
    equityData.length > 0 && equityData[equityData.length - 1].equity >= 0;

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4 }}
      className={cn(
        "bg-[#111111] border border-[#222] rounded-xl p-6",
        "font-mono"
      )}
    >
      <h2 className="text-lg font-semibold text-[#fafafa] mb-4">
        Equity Curve
      </h2>

      {equityData.length === 0 ? (
        <div className="h-64 flex items-center justify-center text-[#888] text-sm">
          No trade data available
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={300}>
          <AreaChart data={equityData}>
            <defs>
              <linearGradient id="equityGradient" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="0%"
                  stopColor={isPositive ? "#00ff88" : "#ff4444"}
                  stopOpacity={0.3}
                />
                <stop
                  offset="100%"
                  stopColor={isPositive ? "#00ff88" : "#ff4444"}
                  stopOpacity={0}
                />
              </linearGradient>
            </defs>
            <XAxis
              dataKey="time"
              tick={{ fill: "#888", fontSize: 11, fontFamily: "JetBrains Mono" }}
              axisLine={{ stroke: "#222" }}
              tickLine={false}
            />
            <YAxis
              tick={{ fill: "#888", fontSize: 11, fontFamily: "JetBrains Mono" }}
              axisLine={{ stroke: "#222" }}
              tickLine={false}
              tickFormatter={(v: number) => formatPnl(v)}
            />
            <Tooltip
              contentStyle={{
                backgroundColor: "#111111",
                border: "1px solid #222",
                borderRadius: 8,
                fontFamily: "JetBrains Mono",
                fontSize: 12,
                color: "#fafafa",
              }}
              formatter={(value: number) => [formatPnl(value), "PnL"]}
              labelStyle={{ color: "#888" }}
            />
            <Area
              type="monotone"
              dataKey="equity"
              stroke={isPositive ? "#00ff88" : "#ff4444"}
              strokeWidth={2}
              fill="url(#equityGradient)"
              dot={false}
              animationDuration={800}
            />
          </AreaChart>
        </ResponsiveContainer>
      )}
    </motion.div>
  );
}
