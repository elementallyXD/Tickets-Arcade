# Contracts Overview

This document describes the core Ticket Arcade smart contracts and their interactions.

---

## Table of Contents

- [High-Level Flow](#high-level-flow)
- [RaffleFactory](#rafflefactory)
- [Raffle](#raffle)
- [Randomness Contracts](#randomness-contracts)
- [Mock Contracts](#mock-contracts-test-only)
- [Lifecycle Diagram](#raffle-lifecycle-summary)

---

## High-Level Flow

```
┌──────────────────┐     ┌──────────────────┐     ┌────────────────────┐
│   RaffleFactory  │────▶│      Raffle      │────▶│ RandomnessProvider │
│   createRaffle() │     │   buyTickets()   │     │ requestRandomness()│
└──────────────────┘     │   finalize()     │     └────────────────────┘
                         └──────────────────┘
```

1. `RaffleFactory.createRaffle()` deploys a new `Raffle` and emits `RaffleCreated`
2. Users buy tickets via `Raffle.buyTickets()` — purchases stored as ranges
3. Raffle closes (time expires or sells out)
4. Creator/keeper requests randomness via the configured `IRandomnessProvider`
5. Provider calls back with randomness and raffle computes the winner
6. Creator/keeper finalizes payout or refunds become available after delay

---

## RaffleFactory

**Purpose**: Deploys per-raffle contracts and enforces global configuration.

### State Variables

| Variable | Type | Description |
|----------|------|-------------|
| `admin` | address | Immutable owner for configuration |
| `usdc` | address | Payment token for all raffles |
| `randomnessProvider` | address | Current provider for new raffles |
| `pendingRandomnessProvider` | address | Scheduled provider update |
| `pendingRandomnessProviderAt` | uint256 | Timestamp when update can apply |
| `maxFeeBps` | uint16 | Global cap on raffle fees |
| `nextRaffleId` | uint256 | Auto-incrementing raffle ID |
| `raffles` | address[] | List of deployed raffle addresses |

### Core Functions

| Function | Access | Description |
|----------|--------|-------------|
| `createRaffle(...)` | Anyone | Deploy new raffle with validated params |
| `rafflesCount()` | View | Returns total raffle count |
| `setRandomnessProvider(addr)` | Admin | Schedule provider update |
| `applyRandomnessProvider()` | Admin | Apply scheduled provider after delay |
| `setMaxFeeBps(bps)` | Admin | Update fee cap |

### Events

- `RaffleCreated` — Emitted on each deployment
- `RandomnessProviderUpdateScheduled` — Provider change initiated
- `RandomnessProviderUpdated` — Provider change applied
- `MaxFeeBpsUpdated` — Fee cap changed

---

## Raffle

**Purpose**: Manages ticket sales, randomness requests, winner selection, and payouts.

### Immutable Configuration

| Variable | Description |
|----------|-------------|
| `raffleId` | Unique ID assigned by factory |
| `usdc` | Payment token address |
| `factory` | Factory that deployed this raffle |
| `creator` | Raffle creator and admin |
| `randomnessProvider` | Provider for randomness requests |
| `feeRecipient` | Address receiving protocol fees |
| `endTime` | Raffle closing timestamp |
| `ticketPrice` | Price per ticket in USDC |
| `maxTickets` | Maximum tickets available |
| `feeBps` | Fee percentage (basis points) |

### Mutable State

| Variable | Description |
|----------|-------------|
| `status` | Lifecycle state (see diagram below) |
| `keeper` | Optional operator for lifecycle actions |
| `totalTickets` | Total tickets sold |
| `pot` | Total USDC collected |
| `ranges[]` | Ordered list of ticket ranges per buyer |
| `requestId` | Randomness request identifier |
| `randomness` | Random value received |
| `winningIndex` | Computed winner index |
| `winner` | Winning address |
| `refunded` | Refund tracking per address |
| `refundsEnabled` | Whether refunds have started |

### Core Functions

| Category | Function | Access |
|----------|----------|--------|
| **Ticketing** | `buyTickets(count)` | Anyone |
| **Lifecycle** | `close()` | Creator / Keeper / Factory |
| | `requestRandom()` | Creator / Keeper |
| | `fulfillRandomness(id, rand)` | Provider only |
| | `finalize()` | Creator / Keeper |
| **Refunds** | `refund()` | Ticket holders |
| | `refundAvailableAt()` | View |
| | `refundAmount(addr)` | View |
| | `canRefund(addr)` | View |
| **Admin** | `setKeeper(addr)` | Creator only |
| **Views** | `rangesCount()` | View |

### Events

| Category | Event |
|----------|-------|
| Ticketing | `TicketsBought` |
| Lifecycle | `RaffleClosed`, `RandomnessRequested`, `RandomnessFulfilled`, `WinnerSelected`, `PayoutsCompleted` |
| Refunds | `RefundClaimed`, `RefundsStarted` |
| Admin | `KeeperUpdated` |

---

## Randomness Contracts

### IRandomnessProvider

**Purpose**: Minimal interface for requesting randomness.

```solidity
interface IRandomnessProvider {
    function requestRandomness(uint256 raffleId) external returns (uint256 requestId);
}
```

### DrandRandomnessProvider

**Purpose**: Adapter for off-chain Drand randomness. Accepts randomness from a trusted oracle and delivers it to raffles.

| Variable | Description |
|----------|-------------|
| `oracle` | Trusted caller for `deliverRandomness` |
| `nextRequestId` | Auto-incrementing request ID |
| `requestToRaffle` | Maps request IDs to raffle addresses |
| `fulfilled` | Tracks completed requests |

| Function | Access | Description |
|----------|--------|-------------|
| `requestRandomness(raffleId)` | Anyone | Returns request ID, records raffle |
| `deliverRandomness(id, rand, proof)` | Oracle only | Fulfills request |

**Events**:
- `RandomnessRequested` — Emitted on request
- `RandomnessDelivered` — Emitted on delivery (includes proof bytes)

---

## Mock Contracts (Test Only)

| Contract | Purpose |
|----------|---------|
| `MockRandomnessProvider` | Same interface, but `fulfill` can be called by anyone |
| `MockUSDC` | Minimal ERC20 with `mint`, `approve`, `transfer`, `transferFrom` |
| `IERC20Minimal` | Minimal ERC20 interface used by Raffle |

---

## Raffle Lifecycle (Summary)

```
                    ┌─────────────────────────┐
                    │         ACTIVE          │
                    │   (accepting tickets)   │
                    └───────────┬─────────────┘
                                │ close()
                    ┌───────────▼─────────────┐
                    │         CLOSED          │
                    │   (no more sales)       │
                    └───────────┬─────────────┘
                                │ requestRandom()
                    ┌───────────▼─────────────┐
                    │    RANDOM_REQUESTED     │
                    │   (waiting for VRF)     │
                    └───────────┬─────────────┘
                                │ fulfillRandomness()
                    ┌───────────▼─────────────┐
                    │    RANDOM_FULFILLED     │
                    │   (winner computed)     │
                    └───────────┬─────────────┘
                                │ finalize()
                    ┌───────────▼─────────────┐
                    │        FINALIZED        │
                    │   (payouts complete)    │
                    └─────────────────────────┘

    ──────────────────────────────────────────────────────
    REFUND PATH: After 48-hour delay from CLOSED or
    RANDOM_REQUESTED, any ticket holder can call refund()
```

---

## See Also

- [SECURITY_MODEL.md](./SECURITY_MODEL.md) — Security analysis and access control
- [../README.md](../README.md) — Quick start and testing guide
