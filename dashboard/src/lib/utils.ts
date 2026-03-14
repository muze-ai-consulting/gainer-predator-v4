import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatPnl(pnl: number): string {
  const sign = pnl >= 0 ? '+' : ''
  return `${sign}${pnl.toFixed(2)}%`
}

export function formatUsd(value: number): string {
  if (value === 0) return '$0.00'
  const abs = Math.abs(value)
  // Smart decimals: more precision for cheaper tokens
  const decimals = abs >= 100 ? 2 : abs >= 1 ? 3 : abs >= 0.01 ? 4 : abs >= 0.0001 ? 6 : 8
  return `$${value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: decimals })}`
}
