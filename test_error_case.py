#!/usr/bin/env python3
"""
Test script for error cases
"""
import asyncio
import websockets
import json
import base64

async def test_error_cases():
    uri = "ws://localhost:9002"
    
    async with websockets.connect(uri) as websocket:
        print("=== Test 1: Invalid cluster ===")
        command = {
            "cluster": "invalid",
            "command": "test",
            "arguments": "base64:e30="
        }
        await websocket.send(json.dumps(command))
        response = await websocket.recv()
        response_json = json.loads(response)
        print(json.dumps(response_json, indent=2))
        if response_json.get("logs"):
            for log in response_json["logs"]:
                print(f"Log: {base64.b64decode(log['message']).decode()}")
        
        print("\n=== Test 2: Invalid base64 encoding ===")
        command = {
            "cluster": "delay",
            "command": "wait-for-commissionee",
            "arguments": "invalid-not-base64"
        }
        await websocket.send(json.dumps(command))
        response = await websocket.recv()
        response_json = json.loads(response)
        print(json.dumps(response_json, indent=2))
        if response_json.get("logs"):
            for log in response_json["logs"]:
                print(f"Log: {base64.b64decode(log['message']).decode()}")
        
        print("\n=== Test 3: Invalid JSON ===")
        await websocket.send("not valid json")
        response = await websocket.recv()
        response_json = json.loads(response)
        print(json.dumps(response_json, indent=2))
        if response_json.get("logs"):
            for log in response_json["logs"]:
                print(f"Log: {base64.b64decode(log['message']).decode()}")

if __name__ == "__main__":
    asyncio.run(test_error_cases())
