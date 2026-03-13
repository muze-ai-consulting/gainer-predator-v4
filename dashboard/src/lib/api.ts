const API_BASE = '/api'

export async function fetchStatus() {
  const res = await fetch(`${API_BASE}/status`)
  return res.json()
}

export async function fetchTrades() {
  const res = await fetch(`${API_BASE}/trades`)
  return res.json()
}

export async function fetchMetrics() {
  const res = await fetch(`${API_BASE}/metrics`)
  return res.json()
}

export async function fetchExperiments() {
  const res = await fetch(`${API_BASE}/experiments`)
  return res.json()
}

export async function postExperiment(params: Record<string, unknown>) {
  const res = await fetch(`${API_BASE}/experiment`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(params),
  })
  return res.json()
}

export interface BotEvent {
  event_type: string
  data: Record<string, unknown>
  timestamp: string
}

export function createEventSource(): EventSource {
  return new EventSource(`${API_BASE}/stream`)
}
