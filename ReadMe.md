# Ticket Arcade

A provably fair raffle system for Arc L1 (EVM). Users purchase USDC tickets, raffles close by time or sellout, verifiable randomness determines a winner, and payouts are finalized on-chain.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Features

- **Provably Fair** — Winner selection uses verifiable randomness from Drand
- **On-Chain Verification** — All raffle logic and payouts are transparent and auditable
- **Refund Protection** — Automatic refund path if randomness delivery fails
- **Flexible Raffles** — Configurable ticket prices, max tickets, end times, and fees

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Smart         │     │   Rust Backend   │     │   React         │
│   Contracts     │────▶│   (Indexer/API)  │────▶│   Frontend      │
│   (Solidity)    │     │                  │     │                 │
└─────────────────┘     └──────────────────┘     └─────────────────┘
        │                        │
        ▼                        ▼
┌─────────────────┐     ┌──────────────────┐
│   Arc L1        │     │   PostgreSQL     │
│   Blockchain    │     │   Database       │
└─────────────────┘     └──────────────────┘
```

## Repository Structure

| Directory | Description |
|-----------|-------------|
| [`contracts/`](contracts/) | Solidity contracts, Hardhat config, and tests |
| [`backend/`](backend/) | Rust indexer and REST API service |
| [`frontend/`](frontend/) | React frontend (planned) |
| [`Documentation/`](Documentation/) | Additional project documentation |

## Quick Start

### Prerequisites

- Node.js 18+ and pnpm
- Rust 1.85+ (2024 edition)
- Docker Desktop
- sqlx-cli (`cargo install sqlx-cli --no-default-features --features postgres`)

### 1. Compile and Test Contracts

```bash
cd contracts
pnpm install
pnpm hardhat compile
pnpm hardhat test
```

See [contracts/README.md](contracts/README.md) for deployment instructions.

### 2. Run Backend

```bash
cd backend
cp .env.example .env
# Edit .env with your deployed contract addresses
docker compose up -d
sqlx migrate run --source migrations
cargo run
```

See [backend/README.md](backend/README.md) for detailed setup.

## Contracts Overview

| Contract | Purpose |
|----------|---------|
| `RaffleFactory.sol` | Deploys per-raffle contracts, enforces fee caps |
| `Raffle.sol` | Ticket sales, randomness requests, winner selection, payouts |
| `DrandRandomnessProvider.sol` | Adapter for Drand verifiable randomness |

### Raffle Lifecycle

```
ACTIVE → CLOSED → RANDOM_REQUESTED → RANDOM_FULFILLED → FINALIZED
           ↘                      ↘
            → Refund path after delay
```

### Key Events

**Factory:** `RaffleCreated`, `RandomnessProviderUpdated`, `MaxFeeBpsUpdated`

**Raffle:** `TicketsBought`, `RaffleClosed`, `RandomnessRequested`, `RandomnessFulfilled`, `WinnerSelected`, `PayoutsCompleted`, `RefundClaimed`

## Documentation

| Document | Description |
|----------|-------------|
| [Contracts Overview](contracts/docs/CONTRACTS_OVERVIEW.md) | Detailed contract architecture |
| [Security Model](contracts/docs/SECURITY_MODEL.md) | Security assumptions and protections |
| [Backend Architecture](backend/docs/ARCHITECTURE.md) | Indexer and API design |
| [API Reference](backend/docs/API.md) | REST API endpoints |
| [Database Schema](backend/docs/DATABASE_SCHEMA.md) | PostgreSQL table structure |
| [Drand Randomization](Documentation/DRAND_Randomization.md) | Randomness provider setup |
| [Testing Guide](backend/TESTING.md) | How to run and write tests |
| [Roadmap](ROADMAP.md) | Project status and future plans |

## Security Considerations

- **Randomness Trust:** The current implementation trusts the Drand oracle. On-chain proof verification is planned for future versions.
- **Access Control:** Admin functions are restricted to the factory admin and raffle creator/keeper.
- **Refund Safety:** Refunds are automatically available after a delay if randomness is not delivered.
- **No OpenZeppelin:** Minimal dependency surface area for reduced attack vectors.

See [contracts/docs/SECURITY_MODEL.md](contracts/docs/SECURITY_MODEL.md) for detailed security analysis.

## Development Status

| Component | Status |
|-----------|--------|
| Smart Contracts | ✅ MVP Complete |
| Backend Indexer | ✅ MVP Complete |
| REST API | ✅ MVP Complete |
| Frontend | 🚧 Planned |

See [ROADMAP.md](ROADMAP.md) for detailed project plans and future features.

## Environment Setup

### Contracts (.env)

```bash
# contracts/.env
ARC_RPC_URL=https://rpc.testnet.arc.network
PRIVATE_KEY=0xYOUR_PRIVATE_KEY_HERE
USDC_ADDRESS=0xYOUR_USDC_ADDRESS
```

> ⚠️ **Security:** Never commit private keys to version control. Use environment variables or a secrets manager.

### Backend (.env)

See [backend/.env.example](backend/.env.example) for all configuration options.

## Contributing

Contributions are welcome! Please open an issue to discuss proposed changes before submitting a pull request.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Contact

- **X (Twitter):** [@TicketArcade](https://x.com/TicketArcade)
- **Email:** ticketarcade.official@gmail.com
- **EVM Address:** `0x3b8059e6A461818bc8F8933428c965b38c5E0bC5`
