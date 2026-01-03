# Ticket Arcade Contracts

Solidity smart contracts for the Ticket Arcade raffle system on Arc L1. Each raffle is deployed as its own contract with provably fair winner selection powered by Drand randomness.

## Features

- Per-raffle contract isolation
- Verifiable randomness via Drand provider
- Automatic refund path if randomness fails
- Configurable fees with global caps
- Delayed randomness provider updates for security

## Prerequisites

- Node.js 18+
- pnpm

## Installation

```bash
cd contracts
pnpm install
```

## Compile

```bash
pnpm hardhat compile
```

## Run Tests

```bash
pnpm hardhat test
```

## Deploy to Arc Testnet

```bash
pnpm deploy:arc
```

### Environment Variables

Create `contracts/.env` before deploying:

```bash
# Required
ARC_RPC_URL=https://rpc.testnet.arc.network
PRIVATE_KEY=0xYOUR_PRIVATE_KEY_HERE
USDC_ADDRESS=0xYOUR_USDC_TOKEN_ADDRESS

# Optional
ORACLE_ADDRESS=0xYOUR_ORACLE_ADDRESS  # Defaults to deployer
MAX_FEE_BPS=500                       # Default: 500 (5%), max: 2000 (20%)
```

> ⚠️ **Security:** Never commit private keys to version control. Use environment variables or a secrets manager for production deployments.

## Network Configuration

| Network | Chain ID | RPC URL |
|---------|----------|---------|
| Arc Testnet | `5042002` | `https://rpc.testnet.arc.network` |

## Project Structure

```
contracts/
├── contracts/           # Solidity source files
│   ├── Raffle.sol
│   ├── RaffleFactory.sol
│   ├── DrandRandomnessProvider.sol
│   └── ...
├── test/                # TypeScript tests
├── scripts/             # Deployment scripts
├── artifacts/           # Compiled artifacts (generated)
└── docs/                # Contract documentation
```

## Documentation

- [Contracts Overview](docs/CONTRACTS_OVERVIEW.md) — Detailed contract architecture
- [Security Model](docs/SECURITY_MODEL.md) — Security assumptions and protections

## Security Considerations

- **Randomness Trust:** The oracle delivering Drand randomness must be trusted. On-chain proof verification is not yet implemented.
- **Access Control:** Only the factory admin can update the randomness provider (with a time delay).
- **Token Assumptions:** The USDC token must be a well-behaved ERC20 that returns `true` on transfers.
- **No Upgradeability:** Raffle contracts are immutable once deployed.

## Tooling

- Hardhat v3
- ethers v6
- TypeScript + Mocha tests
- Solidity ^0.8.24

## License

MIT
