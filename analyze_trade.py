import urllib.request
import json
from datetime import datetime, timezone

def fetch_klines(symbol, start_time_str, end_time_str):
    start_dt = datetime.strptime(start_time_str, "%Y-%m-%d %H:%M:%S").replace(tzinfo=timezone.utc)
    end_dt = datetime.strptime(end_time_str, "%Y-%m-%d %H:%M:%S").replace(tzinfo=timezone.utc)

    start_ts = int(start_dt.timestamp() * 1000)
    end_ts = int(end_dt.timestamp() * 1000)

    url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval=1m&startTime={start_ts}&endTime={end_ts}&limit=100"
    
    req = urllib.request.Request(url)
    with urllib.request.urlopen(req) as response:
        data = json.loads(response.read().decode())
    
    print(f"--- 1m Klines for {symbol} ---")
    print(f"{'Time':<20} | {'Open':<8} | {'High':<8} | {'Low':<8} | {'Close':<8}")
    print("-" * 60)
    for row in data:
        t = datetime.fromtimestamp(row[0]/1000, tz=timezone.utc).strftime('%Y-%m-%d %H:%M:%S')
        o, h, l, c = float(row[1]), float(row[2]), float(row[3]), float(row[4])
        print(f"{t:<20} | {o:<8.4f} | {h:<8.4f} | {l:<8.4f} | {c:<8.4f}")

def simulate_apex_klines(symbol, start_time_str, end_time_str, entry_price):
    start_dt = datetime.strptime(start_time_str, "%Y-%m-%d %H:%M:%S").replace(tzinfo=timezone.utc)
    end_dt = datetime.strptime(end_time_str, "%Y-%m-%d %H:%M:%S").replace(tzinfo=timezone.utc)
    
    start_ts = int(start_dt.timestamp() * 1000)
    end_ts = int(end_dt.timestamp() * 1000)

    url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval=1m&startTime={start_ts}&endTime={end_ts}&limit=100"
    
    req = urllib.request.Request(url)
    try:
        with urllib.request.urlopen(req) as response:
            klines = json.loads(response.read().decode())
    except Exception as e:
        print("Error fetching klines:", e)
        return

    highest_price = entry_price
    stop_loss_pct = 0.015 # 1.5%
    apex_retracement_pct = 0.01 # 1.0%

    print(f"\n--- Apex Simulation (Entry: {entry_price}) ---")
    for row in klines:
        time_str = datetime.fromtimestamp(row[0]/1000, tz=timezone.utc).strftime('%H:%M:%S')
        o, h, l, c = float(row[1]), float(row[2]), float(row[3]), float(row[4])
        
        # Simulamos que toca el High primero y luego el Low de esa vela.
        if h > highest_price:
            highest_price = h
            
        sl_price = entry_price * (1 - stop_loss_pct)
        apex_price = highest_price * (1 - apex_retracement_pct)
        
        if l <= sl_price:
            print(f"[{time_str}] STOP LOSS HIT at wick low {l:.4f} (SL was {sl_price:.4f})")
            return
        elif l <= apex_price:
            pnl_base = ((apex_price/entry_price)-1)*100
            print(f"[{time_str}] APEX HIT at {apex_price:.4f} (Highest was {highest_price:.4f})")
            print(f"-> 1.0% drop from {highest_price:.4f} = {apex_price:.4f}")
            print(f"-> Estimated PnL (No Lev): {pnl_base:.2f}% | (20x Lev): {pnl_base*20:.2f}%")
            return

    print("No exit condition hit within the fetched klines.")

if __name__ == "__main__":
    symbol = "RIVERUSDT"
    # The user's local timezone is UTC-3. 00:45 local time is 03:45 UTC.
    start_time = "2026-03-06 03:44:00"
    end_time = "2026-03-06 03:55:00"
    
    fetch_klines(symbol, start_time, end_time)
    
    # Asumimos una entrada en la apertura de la vela de 00:45 local (03:45 UTC)
    print("\n>>> Simulación asumiendo entrada a las 03:45:00 UTC en ~19.092")
    simulate_apex_klines(symbol, "2026-03-06 03:45:00", end_time, 19.092)
