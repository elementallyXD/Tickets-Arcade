# Drand Randomization

This document explains how Ticket Arcade uses [Drand](https://drand.love/) for verifiable randomness in winner selection.

## Overview

Drand is a distributed randomness beacon that produces publicly verifiable random values at regular intervals. The `DrandRandomnessProvider` smart contract acts as an on-chain adapter that:

1. Receives randomness requests from raffles
2. Accepts randomness deliveries from a trusted oracle
3. Forwards the randomness to the requesting raffle

### Winner Selection Formula

```
winningIndex = randomness % totalTickets
```

The winner is determined by finding which ticket range contains the `winningIndex`.

---

## Architecture

```
┌──────────────┐     ┌────────────────────┐     ┌─────────────┐
│   Drand      │────▶│   Off-chain        │────▶│   Drand     │
│   Beacon     │     │   Oracle Service   │     │   Provider  │
└──────────────┘     └────────────────────┘     │   Contract  │
                                                 └──────┬──────┘
                                                        │
                                                        ▼
                                                 ┌─────────────┐
                                                 │   Raffle    │
                                                 │   Contract  │
                                                 └─────────────┘
```

---

## Backend Configuration

### Required Environment Variables

```bash
# Randomness provider contract address (optional but recommended)
RANDOMNESS_PROVIDER_ADDRESS=0xYOUR_PROVIDER_ADDRESS

# Raffle factory address (required)
RAFFLE_FACTORY_ADDRESS=0xYOUR_FACTORY_ADDRESS

# Block to start indexing from (set before your first deployment)
START_BLOCK=0
```

> ⚠️ **Security:** Never commit real private keys or sensitive addresses to version control.

### Indexed Events

When `RANDOMNESS_PROVIDER_ADDRESS` is configured, the backend indexes:

| Event | Source | Purpose |
|-------|--------|---------|
| `RandomnessRequested` | Provider | Track pending requests |
| `RandomnessDelivered` | Provider | Store randomness + proof |
| `RandomnessRequested` | Raffle | Raffle-level request tracking |
| `RandomnessFulfilled` | Raffle | Raffle-level fulfillment |
| `WinnerSelected` | Raffle | Final winner determination |

---

## Randomness Flow

```
1. Raffle closes (time expires or sells out)
        │
        ▼
2. Creator/keeper calls raffle.requestRandom()
        │
        ▼
3. Raffle calls provider.requestRandomness(raffleId)
        │
        ▼
4. Provider emits RandomnessRequested(requestId, raffleId, raffle)
        │
        ▼
5. Off-chain oracle monitors for requests
        │
        ▼
6. Oracle fetches randomness from Drand beacon
        │
        ▼
7. Oracle calls provider.deliverRandomness(requestId, randomness, proof)
        │
        ▼
8. Provider calls raffle.fulfillRandomness(requestId, randomness)
        │
        ▼
9. Raffle computes winningIndex and emits WinnerSelected
```

---

## Verification

Users can independently verify the winner selection:

### API Endpoint

```
GET /v1/raffles/{raffle_id}/proof
```

Returns:
- `randomness` — The random value used
- `total_tickets` — Total tickets sold
- `winning_index` — Computed as `randomness % total_tickets`
- `winner` — Address that owns the winning ticket
- `winning_range` — The ticket range containing the winning index
- `proof_data` — Drand proof (if available)
- Transaction links for full on-chain audit

### Manual Verification

```javascript
// Verify the winner selection
const winningIndex = BigInt(randomness) % BigInt(totalTickets);

// Find the winning range
const winnerRange = purchases.find(p => 
  p.start_index <= winningIndex && p.end_index >= winningIndex
);

// winnerRange.buyer should match the declared winner
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| No randomness events | Verify oracle is running and connected to provider |
| Wrong chain | Check `CHAIN_ID` and `RPC_URL` in `.env` |
| Missing historical data | Set `START_BLOCK` before contract deployment |
| Provider not indexed | Ensure `RANDOMNESS_PROVIDER_ADDRESS` is set |
| Stale data | Restart backend after changing provider address |

---

## Security Considerations

| Aspect | Status |
|--------|--------|
| Randomness source | Drand beacon (publicly verifiable) |
| On-chain proof verification | ❌ Not implemented (oracle trusted) |
| Oracle reliability | Single point of failure |
| Fallback mechanism | Refunds available after 48-hour delay |

### Future Improvements

- On-chain BLS signature verification of Drand proofs
- Multiple oracle redundancy
- Threshold signature requirements

---

## Notes

- The backend **does not generate randomness** — it only indexes on-chain events
- If the provider address changes, update `.env` and restart the backend
- The oracle is the trust bottleneck — ensure it runs on reliable infrastructure
