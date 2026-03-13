import { useEffect, useRef, useState, useCallback } from 'react'
import type { BotEvent } from '@/lib/api'

export function useSSE() {
  const [events, setEvents] = useState<BotEvent[]>([])
  const [connected, setConnected] = useState(false)
  const esRef = useRef<EventSource | null>(null)
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const connect = useCallback(() => {
    if (esRef.current) {
      esRef.current.close()
      esRef.current = null
    }

    const es = new EventSource('/api/stream')
    esRef.current = es

    es.onopen = () => setConnected(true)

    es.onerror = () => {
      setConnected(false)
      es.close()
      esRef.current = null
      // Reconnect with backoff
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current)
      reconnectTimer.current = setTimeout(connect, 3000)
    }

    es.onmessage = (e) => {
      setConnected(true)
      try {
        const event: BotEvent = JSON.parse(e.data)
        setEvents(prev => [...prev.slice(-200), event])
      } catch { /* ignore parse errors */ }
    }
  }, [])

  // Also poll /api/status to detect bot availability for the "connected" indicator
  useEffect(() => {
    const checkAlive = async () => {
      try {
        const res = await fetch('/api/status')
        if (res.ok) setConnected(true)
      } catch { /* bot not available */ }
    }
    const id = setInterval(checkAlive, 5000)
    return () => clearInterval(id)
  }, [])

  useEffect(() => {
    connect()
    return () => {
      esRef.current?.close()
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current)
    }
  }, [connect])

  return { events, connected }
}
