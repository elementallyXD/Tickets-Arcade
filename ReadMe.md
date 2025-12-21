# Ticket Arcade

Ticket Arcade is a provably fair raffle system for Arc L1 (EVM). Users buy USDC tickets, raffles close by time or sellout, randomness is requested, a winner is selected on-chain, and payouts are finalized on-chain.

## Repo structure
- `backend/`: Rust service placeholder (indexer/API will live here).
- `contracts/`: Solidity contracts, Hardhat v3 config, and tests.
- `docs/`: Project documentation (empty for now).
- `frontend/`: Frontend app placeholder (React planned).

## Contracts overview
Main contracts (in `contracts/contracts/`):
- `RaffleFactory.sol`: deploys a `Raffle` per raffle, enforces fee caps, and uses a delayed randomness provider update.
- `Raffle.sol`: ticket sales, close -> randomness request -> fulfill -> finalize flow, refund path if VRF stalls, creator/keeper permissions.

Key behaviors:
- Tickets are sold in contiguous ranges to make winner verification deterministic.
- Refunds are available after `REFUND_DELAY` if randomness is not fulfilled.
- Randomness provider updates are scheduled and applied after a delay.

Events used by the backend/indexer:
- Factory: `RaffleCreated`, `RandomnessProviderUpdateScheduled`, `RandomnessProviderUpdated`, `MaxFeeBpsUpdated`
- Raffle: `TicketsBought`, `RaffleClosed`, `RandomnessRequested`, `RandomnessFulfilled`, `WinnerSelected`, `PayoutsCompleted`, `RefundClaimed`, `KeeperUpdated`

## Contract state machine
```
ACTIVE -> CLOSED -> RANDOM_REQUESTED -> RANDOM_FULFILLED -> FINALIZED
           \                         \
            \                         -> refund path after delay
             -> refund path after delay
```

## Quick start (contracts)
From `contracts/`:
```
pnpm install
pnpm hardhat compile
pnpm hardhat test mocha
pnpm hardhat clean
```

## Environment
Create `contracts/.env` for testnet runs:
```
ARC_RPC_URL=...
PRIVATE_KEY=...
```
Local tests do not require a `.env`.

## Current status
- Contracts: MVP complete with tests.
- Backend: placeholder Rust crate.
- Frontend: placeholder directory.

## Notes
- Solidity version: ^0.8.24
- Tooling: Hardhat v3, ethers v6, Mocha tests
- No OpenZeppelin dependencies (minimal surface area).

## Contact
For questions or contributions, please open an issue or contact the maintainer.
Other ways to reach me are below:
# X (Twitter): [@TicketArcade](https://x.com/TicketArcade)
# Email: ticketarcade.official@gmail.com
# EVM Address: 0x3b8059e6A461818bc8F8933428c965b38c5E0bC5
