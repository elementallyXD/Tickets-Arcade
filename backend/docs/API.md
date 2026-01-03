# API Reference

Base URL (local): `http://localhost:8080`

All responses are JSON. Numeric token values are returned as **strings** to preserve precision.

## Health
**GET** `/health`

Response:
```json
{ "status": "ok" }
```

## List raffles
**GET** `/v1/raffles`

Query parameters:
- `limit` (optional, default 50, max 100)
- `offset` (optional, default 0)
- `status` (optional, filter by raffle status)

Response (example):
```json
[
  {
    "raffle_id": 1,
    "raffle_address": "0xabc...",
    "status": "ACTIVE",
    "end_time": "2025-01-01T12:00:00Z",
    "ticket_price": "1000000",
    "total_tickets": 42,
    "pot": "42000000",
    "winner": null
  }
]
```

Errors:
- `400` invalid `limit` or `offset`
- `500` internal error

## Get raffle details
**GET** `/v1/raffles/{raffle_id}`

Response (example):
```json
{
  "raffle_id": 1,
  "raffle_address": "0xabc...",
  "creator": "0xcreator...",
  "end_time": "2025-01-01T12:00:00Z",
  "ticket_price": "1000000",
  "max_tickets": 1000,
  "fee_bps": 500,
  "fee_recipient": "0xfee...",
  "status": "RANDOM_FULFILLED",
  "total_tickets": 500,
  "pot": "500000000",
  "request_id": "12",
  "request_tx": "0xreqtx...",
  "randomness": "123456789",
  "randomness_tx": "0xrandtx...",
  "winning_index": 37,
  "winner": "0xwinner...",
  "finalized_tx": "0xfinal..."
}
```

Errors:
- `404` raffle not found
- `500` internal error

## List purchases (ticket ranges)
**GET** `/v1/raffles/{raffle_id}/purchases`

Query parameters:
- `limit` (optional, default 50, max 100)
- `offset` (optional, default 0)

Response (example):
```json
[
  {
    "buyer": "0xbuyer...",
    "start_index": 0,
    "end_index": 9,
    "count": 10,
    "amount": "10000000",
    "tx_hash": "0xtx...",
    "log_index": 3,
    "block_number": 17542050,
    "created_at": "2025-01-01T12:05:00Z"
  }
]
```

Errors:
- `400` invalid `limit` or `offset`
- `500` internal error

## Raffle proof
**GET** `/v1/raffles/{raffle_id}/proof`

Response (example):
```json
{
  "raffle_id": 1,
  "raffle_address": "0xraffle...",
  "request_id": "12",
  "provider_request_id": "45",
  "randomness": "123456789",
  "proof_data": "0xproof...",
  "total_tickets": 500,
  "winning_index": 37,
  "winner": "0xwinner...",
  "winning_range": {
    "buyer": "0xbuyer...",
    "start_index": 30,
    "end_index": 39
  },
  "txs": {
    "request_tx": "0xreqtx...",
    "request_url": "https://testnet.arcscan.app/tx/0xreqtx...",
    "randomness_tx": "0xrandtx...",
    "randomness_url": "https://testnet.arcscan.app/tx/0xrandtx...",
    "finalized_tx": "0xfinal...",
    "finalized_url": "https://testnet.arcscan.app/tx/0xfinal...",
    "provider_request_tx": "0xprovreq...",
    "provider_request_url": "https://testnet.arcscan.app/tx/0xprovreq...",
    "provider_fulfill_tx": "0xprovful...",
    "provider_fulfill_url": "https://testnet.arcscan.app/tx/0xprovful..."
  }
}
```

Notes:
- `winning_index` may be recomputed from `randomness` when missing in DB.
- `winning_range` is derived from stored ticket ranges.
- Provider fields (`provider_*`) are populated when `RANDOMNESS_PROVIDER_ADDRESS` is configured.

Errors:
- `404` raffle not found
- `500` internal error

---

## Randomness Provider Endpoints

These endpoints are available when `RANDOMNESS_PROVIDER_ADDRESS` is configured.

## List randomness requests
**GET** `/v1/randomness/requests`

Query parameters:
- `limit` (optional, default 50, max 100)
- `offset` (optional, default 0)
- `raffle_address` (optional, filter by raffle contract address)
- `raffle_id` (optional, filter by raffle ID)

Response (example):
```json
[
  {
    "id": 1,
    "request_id": "45",
    "raffle_id": 1,
    "raffle_address": "0xraffle...",
    "provider_address": "0xprovider...",
    "tx_hash": "0xtx...",
    "tx_url": "https://testnet.arcscan.app/tx/0xtx...",
    "log_index": 2,
    "block_number": 17542100,
    "created_at": "2025-01-01T12:10:00Z"
  }
]
```

Errors:
- `400` invalid `limit` or `offset`
- `500` internal error

## Get randomness request by ID
**GET** `/v1/randomness/requests/{request_id}`

Response: Same structure as list item above.

Errors:
- `404` randomness request not found
- `500` internal error

## List randomness fulfillments
**GET** `/v1/randomness/fulfillments`

Query parameters:
- `limit` (optional, default 50, max 100)
- `offset` (optional, default 0)
- `raffle_address` (optional, filter by raffle contract address)

Response (example):
```json
[
  {
    "id": 1,
    "request_id": "45",
    "randomness": "123456789012345678901234567890",
    "proof": "0xproof...",
    "raffle_address": "0xraffle...",
    "provider_address": "0xprovider...",
    "tx_hash": "0xtx...",
    "tx_url": "https://testnet.arcscan.app/tx/0xtx...",
    "log_index": 3,
    "block_number": 17542150,
    "created_at": "2025-01-01T12:15:00Z"
  }
]
```

Errors:
- `400` invalid `limit` or `offset`
- `500` internal error
