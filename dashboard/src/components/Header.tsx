import { motion } from 'framer-motion'
import { cn } from '@/lib/utils'

interface HeaderProps {
  connected: boolean
}

export function Header({ connected }: HeaderProps) {
  return (
    <header className="flex items-center justify-between px-6 py-4 border-b border-[#222]">
      <div className="flex items-center gap-4">
        <h1 className="text-xl font-bold tracking-tight text-[#fafafa] font-mono">
          GAINER PREDATOR V4
        </h1>
        <span className="px-2 py-0.5 text-xs font-mono font-semibold rounded bg-[#ffcc00]/10 text-[#ffcc00] border border-[#ffcc00]/20">
          PAPER
        </span>
      </div>

      <div className="flex items-center gap-2">
        <motion.div
          className={cn(
            'h-2.5 w-2.5 rounded-full',
            connected ? 'bg-[#00ff88]' : 'bg-[#ff4444]'
          )}
          animate={connected ? {
            boxShadow: [
              '0 0 0px rgba(0,255,136,0.4)',
              '0 0 8px rgba(0,255,136,0.6)',
              '0 0 0px rgba(0,255,136,0.4)',
            ],
          } : {}}
          transition={{
            duration: 1.5,
            repeat: Infinity,
            ease: 'easeInOut',
          }}
        />
        <span className={cn(
          'text-xs font-mono',
          connected ? 'text-[#00ff88]' : 'text-[#ff4444]'
        )}>
          {connected ? 'LIVE' : 'DISCONNECTED'}
        </span>
      </div>
    </header>
  )
}
