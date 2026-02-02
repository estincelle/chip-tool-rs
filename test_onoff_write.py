#!/usr/bin/env python3
"""
Test script for onoff write command
"""
import asyncio
import websockets
import json
import base64

async def test_onoff_write():
    uri = "ws://localhost:9002"
    
    print("=== Testing onoff write command ===\n")
    
    async with websockets.connect(uri) as websocket:
        # Create the command message matching the YAML test runner format
        args_json = json.dumps({
            "destination-id": "0x12344321",
            "endpoint-id-ignored-for-group-commands": "1",
            "attribute-values": "30"
        })
        args_base64 = "base64:" + base64.b64encode(args_json.encode()).decode()
        
        command = {
            "cluster": "onoff",
            "command": "write",
            "arguments": args_base64,
            "command_specifier": "on-time"
        }
        
        # Add json: prefix like the YAML test runner does
        message = "json:" + json.dumps(command)
        
        print(f"Sending command:")
        print(f"  {message}\n")
        
        await websocket.send(message)
        response = await websocket.recv()
        
        print(f"Received response:")
        print(f"  {response}\n")
        
        response_json = json.loads(response)
        
        print("=== Parsed Response ===")
        print(json.dumps(response_json, indent=2))
        
        # Decode log messages
        if "logs" in response_json:
            print("\n=== Decoded Log Messages ===")
            for log in response_json["logs"]:
                decoded_message = base64.b64decode(log["message"]).decode()
                print(f"[{log['category']}] {decoded_message}")
        
        # Check results
        if "results" in response_json and response_json["results"]:
            print("\n=== Results ===")
            for result in response_json["results"]:
                if "error" in result:
                    print(f"❌ Error: {result['error']}")
                    return False
                else:
                    print(f"Cluster: {result.get('clusterId')}")
                    print(f"Endpoint: {result.get('endpointId')}")
                    print(f"Attribute: {result.get('attributeId')}")
                    print(f"Success: No error field present")
        
        print("\n✅ onoff write command SUCCESSFUL")
        return True

if __name__ == "__main__":
    success = asyncio.run(test_onoff_write())
    if not success:
        exit(1)
