#!/usr/bin/env python3
"""
Test that the server handles the json: prefix used by YAML test runner
"""
import asyncio
import websockets
import json
import base64

async def test_json_prefix():
    uri = "ws://localhost:9002"
    
    print("=== Testing json: prefix handling ===\n")
    
    async with websockets.connect(uri) as websocket:
        # Test with json: prefix (as used by YAML test runner)
        args_json = json.dumps({"nodeId": "305414945"})
        args_base64 = "base64:" + base64.b64encode(args_json.encode()).decode()
        command = {
            "cluster": "delay",
            "command": "wait-for-commissionee",
            "arguments": args_base64
        }
        
        # Add json: prefix like the YAML test runner does
        message_with_prefix = "json:" + json.dumps(command)
        
        print(f"Sending message with json: prefix:")
        print(f"  {message_with_prefix}\n")
        
        await websocket.send(message_with_prefix)
        response = await websocket.recv()
        
        print(f"Received response:")
        print(f"  {response}\n")
        
        response_json = json.loads(response)
        
        # Check if successful
        if not response_json.get("results") or not any("error" in r for r in response_json["results"]):
            print("✅ Server successfully handled json: prefix")
            if response_json.get("logs"):
                decoded = base64.b64decode(response_json["logs"][0]["message"]).decode()
                print(f"✅ Log message: {decoded}")
            return True
        else:
            print("❌ Server returned an error")
            return False

if __name__ == "__main__":
    success = asyncio.run(test_json_prefix())
    if success:
        print("\n✅ json: prefix test PASSED")
    else:
        print("\n❌ json: prefix test FAILED")
        exit(1)
