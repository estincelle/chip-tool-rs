#!/usr/bin/env python3
"""
Simple WebSocket client to test the chip-tool-rs server
"""
import asyncio
import websockets
import sys

async def test_client():
    uri = "ws://localhost:9002"
    
    try:
        async with websockets.connect(uri) as websocket:
            print(f"Connected to {uri}")
            
            # Send some test messages
            messages = [
                "Hello from test client",
                "This is a test message",
                '{"command": "test", "data": "json payload"}',
                "Final test message"
            ]
            
            for msg in messages:
                print(f"Sending: {msg}")
                await websocket.send(msg)
                await asyncio.sleep(0.5)
            
            print("All messages sent successfully")
            await websocket.close()
            print("Connection closed")
            
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(test_client())
