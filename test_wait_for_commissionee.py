#!/usr/bin/env python3
"""
Test script for wait-for-commissionee command
"""
import asyncio
import websockets
import json
import base64
import sys

async def test_wait_for_commissionee():
    uri = "ws://localhost:9002"
    
    try:
        async with websockets.connect(uri) as websocket:
            print(f"Connected to {uri}")
            
            # Create the command message matching chip-tool format
            # The arguments are base64-encoded JSON: { "nodeId":"305414945" }
            args_json = json.dumps({"nodeId": "305414945"})
            args_base64 = "base64:" + base64.b64encode(args_json.encode()).decode()
            
            command = {
                "cluster": "delay",
                "command": "wait-for-commissionee",
                "arguments": args_base64
            }
            
            print("\n=== Sending Command ===")
            print(json.dumps(command, indent=2))
            
            # Send the command
            await websocket.send(json.dumps(command))
            print("\nCommand sent, waiting for response...")
            
            # Receive the response
            response = await websocket.recv()
            print("\n=== Received Response ===")
            print(response)
            
            # Parse and pretty-print the response
            response_json = json.loads(response)
            print("\n=== Parsed Response ===")
            print(json.dumps(response_json, indent=2))
            
            # Decode the log messages
            if "logs" in response_json:
                print("\n=== Decoded Log Messages ===")
                for log in response_json["logs"]:
                    decoded_message = base64.b64decode(log["message"]).decode()
                    print(f"[{log['category']}] {decoded_message}")
            
            # Check if it was successful
            if response_json.get("results") and any("error" in r for r in response_json["results"]):
                print("\n❌ Command FAILED")
                sys.exit(1)
            else:
                print("\n✅ Command SUCCESSFUL")
            
            await websocket.close()
            
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(test_wait_for_commissionee())
