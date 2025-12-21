
## Project Structure
The project is organized into the following main directories:
- backend
- contracts
- docs
- fornted

## Contracts
This directory contains all the smart contract code for the project. It includes:
- `contracts/`: The main smart contract files.
- `migrations/`: Scripts for deploying the contracts.
- `test/`: Unit tests for the smart contracts.
- `scripts/`: Utility scripts for interacting with the contracts.
- `hardhat.config.js`: Configuration file for the Hardhat development environment.
- `package.json`: Node.js package file listing dependencies and scripts.

Contracts are written in Solidity and are designed to be deployed on the Arc blockchain.

## Raffle Contract State Machine
The Raffle contract operates through a defined state machine with the following states and transitions:
```
ACTIVE -> CLOSED -> RANDOM_REQUESTED -> RANDOM_FULFILLED -> FINALIZED
                  \                    \
                   \                    -> (refund path if delay passed)
                    -> (refund path if delay passed)
```

**States:**
`ACTIVE`: The raffle is open for ticket purchases.
`CLOSED`: The raffle is closed for ticket purchases.
`RANDOM_REQUESTED`: A request for randomness has been made to determine the winner.
`RANDOM_FULFILLED`: Randomness has been received and the winner can be determined.
`FINALIZED`: The raffle has been finalized and the winner has been awarded.
`Refund Path`: If a certain delay has passed without randomness being fulfilled, refunds can be enabled, allowing participants to reclaim their tickets.

**Valid Transitions:**
- `ACTIVE` → `CLOSED`: via `close()` or auto-close on sold out
- `CLOSED` → `RANDOM_REQUESTED`: via `requestRandom()`
- `RANDOM_REQUESTED` → `RANDOM_FULFILLED`: via `fulfillRandomness()`
- `RANDOM_FULFILLED` → `FINALIZED`: via `finalize()`

**Refund Path:**
- From `CLOSED` or `RANDOM_REQUESTED`: If `endTime + REFUND_DELAY` has passed
- Once any refund occurs, `refundsEnabled = true` blocks `fulfillRandomness()` and `finalize()`

