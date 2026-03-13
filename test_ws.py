import asyncio
import websockets
import json

async def test():
    async with websockets.connect("wss://fstream.binance.com/ws/!bookTicker") as ws:
        msg = await ws.recv()
        print(json.loads(msg))
        msg = await ws.recv()
        print(json.loads(msg))

asyncio.run(test())
