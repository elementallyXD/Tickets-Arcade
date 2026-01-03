# DRAND Randomization

This project uses a **Drand Randomness Provider** smart contract to supply verifiable randomness
to raffles. The backend only needs the provider contract address in `.env` to index randomness
events and expose proof data.

## What is Drand in this project?
Drand is an external randomness source. The on-chain contract
`DrandRandomnessProvider` receives randomness from a trusted oracle and
calls the raffle contract with the random value. The raffle computes:

`winningIndex = randomness % totalTickets`

## Quick setup (backend)

1) Deploy or use an existing DrandRandomnessProvider contract.
2) Put the address in your backend `.env`:
   ```
   RANDOMNESS_PROVIDER_ADDRESS=0x...
   ```
3) Also set the raffle factory:
   ```
   RAFFLE_FACTORY_ADDRESS=0x...
   ```
4) Set a start block for indexing:
   ```
   START_BLOCK=123456
   ```
5) Run the backend. The indexer will read:
   - RandomnessRequested
   - RandomnessFulfilled
   - WinnerSelected

The API will then serve `/v1/raffles/{id}/proof`.

## Example (Arc testnet deployment)
These are example values from a testnet deployment and can be replaced:
```
RANDOMNESS_PROVIDER_ADDRESS=0x4af4721E1339DAb5C2484045d401C4A4290320C6
RAFFLE_FACTORY_ADDRESS=0x8895f9297570B6199BC617885973F5790Fa773A4
START_BLOCK=17542046
```

## How the randomness flows (simple)
1) Raffle closes.
2) `requestRandom()` is called on the raffle.
3) Drand provider emits `RandomnessRequested`.
4) Oracle delivers randomness to the Drand provider.
5) Drand provider calls `raffle.fulfillRandomness(requestId, randomness)`.
6) Raffle emits `RandomnessFulfilled`.
7) Raffle computes the winner index and emits `WinnerSelected`.

## Verify in the UI
The frontend (or user) can verify:
- randomness
- totalTickets
- winningIndex = randomness % totalTickets
- which ticket range owns winningIndex

## Troubleshooting
- No randomness events: confirm the oracle is sending to the Drand provider.
- Wrong chain: verify `CHAIN_ID` and `RPC_URL` point to the right network.
- Missing data: ensure `START_BLOCK` is at or before the contract deployment.

## Notes
- The backend **does not** generate randomness. It only reads on-chain events.
- If the Drand address changes, update `RANDOMNESS_PROVIDER_ADDRESS` and restart the backend.
