# Ticket Arcade Roadmap

This document outlines the project status, planned improvements, and future directions.

---

## Current Status: MVP Complete ✅

### Backend
- [x] Blockchain event indexer (Axum + ethers-rs)
- [x] PostgreSQL storage with migrations
- [x] REST API for raffles, purchases, and proofs
- [x] DrandRandomnessProvider integration
- [x] Graceful shutdown handling
- [x] Configurable batch sizing and polling

### Smart Contracts
- [x] RaffleFactory with admin controls
- [x] Raffle contract with full lifecycle
- [x] DrandRandomnessProvider for verifiable randomness
- [x] Refund mechanism with time delay
- [x] Comprehensive test suite (e2e, ranges, refunds)

### Infrastructure
- [x] Docker Compose for local Postgres
- [x] Environment-based configuration
- [x] Arc L1 testnet deployment

---

## Phase 1: Production Hardening

### Backend Improvements
- [ ] Add request rate limiting (tower middleware)
- [ ] Implement caching layer (Redis or in-memory)
- [ ] Add metrics endpoint (Prometheus format)
- [ ] Health check with database ping
- [ ] Structured JSON logging
- [ ] Connection pooling tuning

### Security
- [ ] API authentication for admin endpoints
- [ ] CORS configuration
- [ ] Input validation hardening
- [ ] Audit logging for state changes

### DevOps
- [ ] CI/CD pipeline (GitHub Actions)
- [ ] Docker image builds
- [ ] Kubernetes deployment manifests
- [ ] Secrets management (Vault or similar)

---

## Phase 2: Feature Expansion

### Raffle Enhancements
- [ ] Multi-token support (beyond USDC)
- [ ] Tiered ticket pricing
- [ ] Early bird discounts
- [ ] Referral tracking
- [ ] Bulk ticket purchase discounts

### Randomness
- [ ] On-chain BLS verification of Drand proofs
- [ ] Multiple oracle redundancy
- [ ] Chainlink VRF v2.5 alternative

### API Extensions
- [ ] WebSocket for live updates
- [ ] GraphQL endpoint
- [ ] Webhook notifications
- [ ] Historical analytics endpoint

---

## Phase 3: Frontend Development

### Core Features
- [ ] Raffle listing with filters
- [ ] Raffle detail view with live ticket count
- [ ] Ticket purchase flow (wallet connect)
- [ ] User ticket history
- [ ] Winner verification page

### User Experience
- [ ] Mobile-responsive design
- [ ] Dark/light theme
- [ ] Transaction status toasts
- [ ] Loading states and skeletons
- [ ] Error handling with retry

### Tech Stack (Suggested)
- **Framework**: React 18 or Next.js 14
- **State**: TanStack Query (React Query)
- **Wallet**: wagmi + viem
- **Styling**: Tailwind CSS
- **Components**: shadcn/ui or Radix

### Pages
| Route | Description |
|-------|-------------|
| `/` | Homepage with featured raffles |
| `/raffles` | Paginated raffle listing |
| `/raffles/:id` | Raffle detail + purchase |
| `/raffles/:id/proof` | Winner verification |
| `/my-tickets` | User's purchased tickets |
| `/create` | Raffle creation (admin) |

---

## Phase 4: Advanced Features

### Governance
- [ ] DAO voting for parameters
- [ ] Fee distribution to stakers
- [ ] Community-proposed raffles

### Analytics
- [ ] Dashboard for raffle creators
- [ ] Historical win rates
- [ ] Volume and revenue tracking
- [ ] User engagement metrics

### Integrations
- [ ] ENS/domain name resolution
- [ ] IPFS for raffle metadata
- [ ] Social sharing (Twitter/Discord)
- [ ] Email notifications

---

## Technical Debt

### Known Issues
- [ ] Indexer restart doesn't validate already-processed events
- [ ] No retry logic for failed RPC calls
- [ ] Large purchase histories not paginated in proof endpoint

### Code Quality
- [ ] Increase test coverage
- [ ] Add benchmarks for indexer performance
- [ ] Document internal APIs
- [ ] Refactor database layer into repository pattern

---

## Contributing

1. Pick an item from the roadmap
2. Create a feature branch
3. Submit a PR with tests
4. Update relevant documentation

---

## Versioning

| Version | Milestone |
|---------|-----------|
| 0.1.0 | MVP backend + contracts |
| 0.2.0 | Production hardening |
| 0.3.0 | Frontend alpha |
| 1.0.0 | Mainnet launch |
