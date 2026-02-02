#!/usr/bin/env python3
"""
Test that state persists across write and read operations
"""
import asyncio
import websockets
import json
import base64

async def test_state_persistence():
    uri = "ws://localhost:9002"
    
    print("=== Testing State Persistence ===\n")
    
    async with websockets.connect(uri) as websocket:
        # Test 1: Write on-time attribute with value 50
        print("--- Step 1: Write on-time = 50 ---")
        args_json = json.dumps({
            "destination-id": "0x12344321",
            "endpoint-id-ignored-for-group-commands": "1",
            "attribute-values": "50"
        })
        args_base64 = "base64:" + base64.b64encode(args_json.encode()).decode()
        
        write_cmd = {
            "cluster": "onoff",
            "command": "write",
            "arguments": args_base64,
            "command_specifier": "on-time"
        }
        
        await websocket.send("json:" + json.dumps(write_cmd))
        write_response = await websocket.recv()
        print(f"Write response: {write_response}\n")
        
        # Test 2: Read on-time attribute and verify it returns 50
        print("--- Step 2: Read on-time (should be 50) ---")
        read_args_json = json.dumps({"destination-id": "0x12344321", "endpoint-ids": "1"})
        read_args_base64 = "base64:" + base64.b64encode(read_args_json.encode()).decode()
        
        read_cmd = {
            "cluster": "onoff",
            "command": "read",
            "arguments": read_args_base64,
            "command_specifier": "on-time"
        }
        
        await websocket.send("json:" + json.dumps(read_cmd))
        read_response = await websocket.recv()
        read_json = json.loads(read_response)
        
        print(f"Read response: {read_response}\n")
        
        # Verify the value
        if read_json.get("results") and len(read_json["results"]) > 0:
            result = read_json["results"][0]
            value = result.get("value")
            
            print(f"=== Verification ===")
            print(f"Expected value: 50")
            print(f"Received value: {value}")
            
            if value == 50:
                print("\n✅ SUCCESS: State persisted correctly!")
                print("   Write set value to 50, and read returned 50")
                return True
            else:
                print(f"\n❌ FAILURE: Expected 50 but got {value}")
                return False
        else:
            print("\n❌ FAILURE: No results in read response")
            return False

if __name__ == "__main__":
    success = asyncio.run(test_state_persistence())
    if not success:
        exit(1)
