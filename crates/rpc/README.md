# TwineBatch JSON-RPC API

These apis are extended to the twine_node if it's run with `twine` feature enabled.

All endpoints are grouped under the **twine** namespace.

| Method | Description |
|--------|-------------|
| `twine_getLatestBatch` | Get the identifier of the most-recently sequenced batch. |
| `twine_getFullBatch` | Fetch complete metadata for a given batch identifier. |
| `twine_getBatchHash` | Get the hash for the batch payload. |
| `twine_getBatchNumberForBlock` | Find the batch identifier that contains an L2 block number. |
| `twine_getBlocksInBatch` | Return the inclusive range of L2 block numbers that belong to a batch. |

## Queries
### twine_getLatestBatch
Request:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "twine_getLatestBatch",
    "params": []
}
```

Response:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": 42
}
```

### twine_getFullBatch
Request:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "twine_getFullBatch",
    "params": [
        42
    ]
}
```
Response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "block_range": {
      "start": 41,
      "end": 50
    },
    "created_at": 1753404588,
    "prev_batch_hash": "0xb51f9361553aeae21046724f0c81470cc241a2ad60a00c4ade379e718e818f7e",
    "batch_hash": "0x8b9b05b08d3cc825306c20c5131c08e1bb515ed7498d534b7fb0eba4fc4418c0",
    "block_metadata": [
      {
        "height": 41,
        "block_hash": "0x91eb82e474b9f0309376468ecba4a6fefdf4fa7e1989eb94fa64de5381dfdeea",
        "state_root": "0xea0654ab448aa38baa2f93dd4e2b7d384541e04fda713723a0cfea8f835aead3"
      },
        ...
      {
        "height": 50,
        "block_hash": "0x5b914a66982e8c2b7bee39cdd52774898827963a5dc28bcb29f31c6a568279d0",
        "state_root": "0xea0654ab448aa38baa2f93dd4e2b7d384541e04fda713723a0cfea8f835aead3"
      }
    ]
  }
}
```


### twine_getBatchHash
Request:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "twine_getBatchHash",
    "params": [
        42
    ]
}
```
Response:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": "0x8f14e45f...e4c5"
}
```


### twine_getBatchNumberForBlock
Request:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "twine_getBatchNumberForBlock",
    "params": [
        42
    ]
}
```
Response:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": 5
}
```


### twine_getBlocksInBatch
Request:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "twine_getBlocksInBatch",
    "params": [
        42
    ]
}
```
Response:
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "start": 123400,
        "end": 123456
    }
}
```
