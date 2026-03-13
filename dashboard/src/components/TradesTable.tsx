import { useMemo } from "react";
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  flexRender,
  type ColumnDef,
  type SortingState,
} from "@tanstack/react-table";
import { useState } from "react";
import { motion } from "framer-motion";
import { cn, formatPnl, formatUsd } from "@/lib/utils";
import { fetchTrades } from "@/lib/api";
import { usePolling } from "@/hooks/usePolling";
import { ArrowUpDown } from "lucide-react";

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

function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatHoldTime(openTime: string, closeTime: string): string {
  const ms = new Date(closeTime).getTime() - new Date(openTime).getTime();
  const totalMinutes = Math.floor(ms / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours === 0) return `${minutes}m`;
  return `${hours}h ${minutes}m`;
}

export function TradesTable() {
  const { data: trades } = usePolling<Trade[]>(fetchTrades, 10_000);
  const [sorting, setSorting] = useState<SortingState>([]);

  const columns = useMemo<ColumnDef<Trade>[]>(
    () => [
      {
        accessorKey: "close_time",
        header: "Time",
        cell: ({ getValue }) => (
          <span className="text-[#888]">
            {formatTime(getValue<string>())}
          </span>
        ),
      },
      {
        accessorKey: "symbol",
        header: "Symbol",
        cell: ({ getValue }) => (
          <span className="text-[#fafafa] font-semibold">
            {getValue<string>()}
          </span>
        ),
      },
      {
        accessorKey: "side",
        header: "Side",
        cell: ({ getValue }) => {
          const side = getValue<string>();
          return (
            <span
              className={cn(
                "text-xs px-2 py-0.5 rounded",
                side === "LONG"
                  ? "text-[#00ff88] bg-[#00ff88]/10"
                  : "text-[#ff4444] bg-[#ff4444]/10"
              )}
            >
              {side}
            </span>
          );
        },
      },
      {
        accessorKey: "entry_price",
        header: "Entry",
        cell: ({ getValue }) => (
          <span className="text-[#fafafa]">
            {formatUsd(getValue<number>())}
          </span>
        ),
      },
      {
        accessorKey: "exit_price",
        header: "Exit",
        cell: ({ getValue }) => (
          <span className="text-[#fafafa]">
            {formatUsd(getValue<number>())}
          </span>
        ),
      },
      {
        accessorKey: "pnl_pct",
        header: "PnL%",
        cell: ({ getValue }) => {
          const pnl = getValue<number>();
          return (
            <span
              className={cn(
                "font-semibold",
                pnl >= 0 ? "text-[#00ff88]" : "text-[#ff4444]"
              )}
            >
              {pnl >= 0 ? "+" : ""}
              {pnl.toFixed(2)}%
            </span>
          );
        },
      },
      {
        accessorKey: "exit_reason",
        header: "Exit Reason",
        cell: ({ getValue }) => (
          <span className="text-[#888] text-xs uppercase tracking-wide">
            {getValue<string>()}
          </span>
        ),
      },
      {
        id: "hold_time",
        header: "Hold Time",
        accessorFn: (row) => formatHoldTime(row.open_time, row.close_time),
        cell: ({ getValue }) => (
          <span className="text-[#888]">{getValue<string>()}</span>
        ),
      },
      {
        accessorKey: "leverage",
        header: "Leverage",
        cell: ({ getValue }) => (
          <span className="text-[#3b82f6]">{getValue<number>()}x</span>
        ),
      },
    ],
    []
  );

  const table = useReactTable({
    data: trades ?? [],
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4, delay: 0.1 }}
      className="bg-[#111111] border border-[#222] rounded-xl p-6 font-mono"
    >
      <h2 className="text-lg font-semibold text-[#fafafa] mb-4">
        Trade History
      </h2>

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
                  No trades yet
                </td>
              </tr>
            ) : (
              table.getRowModel().rows.map((row) => {
                const isProfit = row.original.pnl_pct >= 0;
                return (
                  <tr
                    key={row.id}
                    className={cn(
                      "border-b border-[#222]/50 transition-colors",
                      isProfit
                        ? "hover:bg-[#00ff88]/[0.03]"
                        : "hover:bg-[#ff4444]/[0.03]"
                    )}
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
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </motion.div>
  );
}
