import { useMemo, useState } from "react";
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

interface HourData {
  hour: number;
  pnl: number;
}

function getPnlColor(pnl: number, maxAbs: number): string {
  if (maxAbs === 0) return "bg-[#1a1a1a]";
  const intensity = Math.min(Math.abs(pnl) / maxAbs, 1);

  if (pnl > 0) {
    const alpha = Math.round(intensity * 60 + 10);
    return `bg-[#00ff88]/${alpha}`;
  }
  if (pnl < 0) {
    const alpha = Math.round(intensity * 60 + 10);
    return `bg-[#ff4444]/${alpha}`;
  }
  return "bg-[#1a1a1a]";
}

function getPnlStyle(pnl: number, maxAbs: number): React.CSSProperties {
  if (maxAbs === 0 || pnl === 0) {
    return { backgroundColor: "#1a1a1a" };
  }
  const intensity = Math.min(Math.abs(pnl) / maxAbs, 1);
  const alpha = intensity * 0.45 + 0.05;

  if (pnl > 0) {
    return { backgroundColor: `rgba(0, 255, 136, ${alpha})` };
  }
  return { backgroundColor: `rgba(255, 68, 68, ${alpha})` };
}

export function HourHeatmap() {
  const { data: trades } = usePolling<Trade[]>(fetchTrades, 10_000);
  const [hoveredHour, setHoveredHour] = useState<number | null>(null);

  const { hourData, maxAbs } = useMemo(() => {
    const hourMap = new Map<number, number>();
    for (let h = 0; h < 24; h++) hourMap.set(h, 0);

    if (trades) {
      for (const t of trades) {
        const hour = new Date(t.close_time).getUTCHours();
        hourMap.set(hour, (hourMap.get(hour) ?? 0) + t.pnl_usd);
      }
    }

    const data: HourData[] = [];
    let maxAbsVal = 0;
    for (const [hour, pnl] of hourMap) {
      data.push({ hour, pnl: Number(pnl.toFixed(2)) });
      maxAbsVal = Math.max(maxAbsVal, Math.abs(pnl));
    }
    data.sort((a, b) => a.hour - b.hour);

    return { hourData: data, maxAbs: maxAbsVal };
  }, [trades]);

  const hoveredData = hoveredHour !== null
    ? hourData.find((d) => d.hour === hoveredHour)
    : null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4, delay: 0.2 }}
      className="bg-[#111111] border border-[#222] rounded-xl p-6 font-mono"
    >
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-[#fafafa]">
          Hour Heatmap
          <span className="text-xs text-[#888] ml-2 font-normal">UTC</span>
        </h2>
        {hoveredData && (
          <span
            className={cn(
              "text-sm font-semibold transition-colors",
              hoveredData.pnl >= 0 ? "text-[#00ff88]" : "text-[#ff4444]"
            )}
          >
            {hoveredData.hour.toString().padStart(2, "0")}:00 &mdash;{" "}
            {formatPnl(hoveredData.pnl)}
          </span>
        )}
      </div>

      <div className="grid grid-cols-6 gap-2">
        {hourData.map(({ hour, pnl }) => (
          <motion.div
            key={hour}
            whileHover={{ scale: 1.08 }}
            onMouseEnter={() => setHoveredHour(hour)}
            onMouseLeave={() => setHoveredHour(null)}
            style={getPnlStyle(pnl, maxAbs)}
            className={cn(
              "relative rounded-lg p-3 cursor-default transition-all",
              "border border-[#222]/50",
              "flex flex-col items-center justify-center min-h-[60px]"
            )}
          >
            <span className="text-xs text-[#888]">
              {hour.toString().padStart(2, "0")}
            </span>
            <span
              className={cn(
                "text-xs font-semibold mt-0.5",
                pnl > 0
                  ? "text-[#00ff88]"
                  : pnl < 0
                    ? "text-[#ff4444]"
                    : "text-[#555]"
              )}
            >
              {pnl === 0 ? "--" : formatPnl(pnl)}
            </span>
          </motion.div>
        ))}
      </div>
    </motion.div>
  );
}
