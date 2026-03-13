import { useMemo, useState } from "react";
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  flexRender,
  type ColumnDef,
  type SortingState,
} from "@tanstack/react-table";
import {
  ResponsiveContainer,
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  Scatter,
  ScatterChart,
  ZAxis,
} from "recharts";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import { fetchExperiments } from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";
import { ArrowUpDown } from "lucide-react";

interface Experiment {
  id: string;
  description: string;
  status: "KEEP" | "DISCARD";
  score: number;
  timestamp: string;
}

interface ScorePoint {
  time: string;
  score: number;
  status: "KEEP" | "DISCARD";
  fill: string;
}

function formatTimestamp(iso: string): string {
  return new Date(iso).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function ExperimentTable({ experiments }: { experiments: Experiment[] }) {
  const [sorting, setSorting] = useState<SortingState>([]);

  const columns = useMemo<ColumnDef<Experiment>[]>(
    () => [
      {
        accessorKey: "id",
        header: "ID",
        cell: ({ getValue }) => (
          <span className="text-[#3b82f6] text-xs font-semibold">
            {getValue<string>().slice(0, 8)}
          </span>
        ),
      },
      {
        accessorKey: "description",
        header: "Description",
        cell: ({ getValue }) => (
          <span className="text-[#fafafa] text-sm max-w-[200px] truncate block">
            {getValue<string>()}
          </span>
        ),
      },
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ getValue }) => {
          const status = getValue<string>();
          return (
            <span
              className={cn(
                "text-xs px-2.5 py-1 rounded-full font-semibold",
                status === "KEEP"
                  ? "text-[#00ff88] bg-[#00ff88]/10"
                  : "text-[#ff4444] bg-[#ff4444]/10"
              )}
            >
              {status}
            </span>
          );
        },
      },
      {
        accessorKey: "score",
        header: "Score",
        cell: ({ getValue }) => {
          const score = getValue<number>();
          return (
            <span
              className={cn(
                "font-semibold",
                score >= 0.7
                  ? "text-[#00ff88]"
                  : score >= 0.4
                    ? "text-[#ffcc00]"
                    : "text-[#ff4444]"
              )}
            >
              {score.toFixed(3)}
            </span>
          );
        },
      },
      {
        accessorKey: "timestamp",
        header: "Timestamp",
        cell: ({ getValue }) => (
          <span className="text-[#888] text-xs">
            {formatTimestamp(getValue<string>())}
          </span>
        ),
      },
    ],
    []
  );

  const table = useReactTable({
    data: experiments,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id} className="border-b border-[#222]">
              {headerGroup.headers.map((header) => (
                <th
                  key={header.id}
                  className="text-left text-[#888] text-xs uppercase tracking-wider py-3 px-3 cursor-pointer select-none hover:text-[#fafafa] transition-colors"
                  onClick={header.column.getToggleSortingHandler()}
                >
                  <div className="flex items-center gap-1">
                    {flexRender(
                      header.column.columnDef.header,
                      header.getContext()
                    )}
                    <ArrowUpDown className="w-3 h-3 opacity-40" />
                  </div>
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.length === 0 ? (
            <tr>
              <td
                colSpan={columns.length}
                className="text-center text-[#888] py-12 text-sm"
              >
                No experiments yet
              </td>
            </tr>
          ) : (
            table.getRowModel().rows.map((row) => (
              <tr
                key={row.id}
                className="border-b border-[#222]/50 hover:bg-[#fafafa]/[0.02] transition-colors"
              >
                {row.getVisibleCells().map((cell) => (
                  <td key={cell.id} className="py-2.5 px-3">
                    {flexRender(
                      cell.column.columnDef.cell,
                      cell.getContext()
                    )}
                  </td>
                ))}
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}

function ScoreHistoryChart({ experiments }: { experiments: Experiment[] }) {
  const chartData = useMemo<ScorePoint[]>(() => {
    if (!experiments || experiments.length === 0) return [];

    const sorted = [...experiments].sort(
      (a, b) =>
        new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
    );

    return sorted.map((exp) => ({
      time: formatTimestamp(exp.timestamp),
      score: exp.score,
      status: exp.status,
      fill: exp.status === "KEEP" ? "#00ff88" : "#ff4444",
    }));
  }, [experiments]);

  return (
    <div className="mt-6">
      <h3 className="text-sm font-semibold text-[#888] uppercase tracking-wider mb-4">
        Score History
      </h3>
      {chartData.length === 0 ? (
        <div className="h-48 flex items-center justify-center text-[#888] text-sm">
          No experiment data
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={240}>
          <LineChart data={chartData}>
            <XAxis
              dataKey="time"
              tick={{ fill: "#888", fontSize: 10, fontFamily: "JetBrains Mono" }}
              axisLine={{ stroke: "#222" }}
              tickLine={false}
            />
            <YAxis
              domain={[0, 1]}
              tick={{ fill: "#888", fontSize: 10, fontFamily: "JetBrains Mono" }}
              axisLine={{ stroke: "#222" }}
              tickLine={false}
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
              formatter={(value: number, _name: string, props: { payload: ScorePoint }) => [
                `${value.toFixed(3)} (${props.payload.status})`,
                "Score",
              ]}
              labelStyle={{ color: "#888" }}
            />
            <Line
              type="monotone"
              dataKey="score"
              stroke="#3b82f6"
              strokeWidth={2}
              dot={(props: {
                cx: number;
                cy: number;
                payload: ScorePoint;
                index: number;
              }) => {
                const { cx, cy, payload, index } = props;
                return (
                  <circle
                    key={index}
                    cx={cx}
                    cy={cy}
                    r={5}
                    fill={payload.status === "KEEP" ? "#00ff88" : "#ff4444"}
                    stroke="#111111"
                    strokeWidth={2}
                  />
                );
              }}
              animationDuration={800}
            />
          </LineChart>
        </ResponsiveContainer>
      )}
    </div>
  );
}

export function ExperimentPanel() {
  const { data: experiments } = usePolling<Experiment[]>(
    fetchExperiments,
    10_000
  );

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4, delay: 0.3 }}
      className="bg-[#111111] border border-[#222] rounded-xl p-6 font-mono"
    >
      <h2 className="text-lg font-semibold text-[#fafafa] mb-4">
        Experiments
      </h2>

      <ExperimentTable experiments={experiments ?? []} />
      <ScoreHistoryChart experiments={experiments ?? []} />
    </motion.div>
  );
}
