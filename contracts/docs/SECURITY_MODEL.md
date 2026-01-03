# Contracts Security Model

This document describes the security assumptions, access controls, and known limitations of the Ticket Arcade smart contracts.

## Table of Contents

- [Randomness Trust Model](#randomness-trust-model)
- [Access Control](#access-control)
- [Refund Behavior](#refund-behavior)
- [Reentrancy Protections](#reentrancy-protections)
- [Token Assumptions](#token-assumptions)
- [Known Limitations](#known-limitations)
- [Hardening Recommendations](#hardening-recommendations)

---

## Randomness Trust Model

Randomness is delivered by `DrandRandomnessProvider` from a **trusted oracle**.

| Aspect | Current State |
|--------|---------------|
| Randomness source | Drand (off-chain) |
| On-chain verification | ❌ Not implemented |
| Oracle trust | Required |

**Risk:** If the oracle provides biased or incorrect randomness, the winner can be manipulated.

### Future Hardening

- On-chain verification of Drand BLS proofs
- Multiple oracle signatures or threshold schemes
- Timeouts and fallback randomness strategies
- Commit-reveal schemes for additional entropy

---

## Access Control

### RaffleFactory

| Function | Allowed Callers |
|----------|-----------------|
| `setRandomnessProvider()` | `admin` only |
| `applyRandomnessProvider()` | `admin` only |
| `setMaxFeeBps()` | `admin` only |
| `createRaffle()` | Anyone |

### Raffle

| Function | Allowed Callers |
|----------|-----------------|
| `setKeeper()` | `creator` only |
| `requestRandom()` | `creator` or `keeper` |
| `finalize()` | `creator` or `keeper` |
| `close()` | `creator`, `keeper`, or `factory` |
| `fulfillRandomness()` | `randomnessProvider` only |
| `buyTickets()` | Anyone |
| `refund()` | Any buyer with tickets |

---

## Refund Behavior

Refunds provide a safety net if randomness delivery fails.

| Condition | Behavior |
|-----------|----------|
| When available | After `REFUND_DELAY` (48 hours) |
| Eligible states | `CLOSED` or `RANDOM_REQUESTED` |
| First claim effect | Sets `refundsEnabled = true` |
| After refunds start | Finalization is permanently blocked |
| Refund amount | `ticketPrice × ticketsByBuyer` |

---

## Reentrancy Protections

The contracts use a simple reentrancy guard and follow the checks-effects-interactions pattern:

| Function | Protection |
|----------|------------|
| `buyTickets()` | ✅ State updated before transfer |
| `requestRandom()` | ✅ State updated before external call |
| `refund()` | ✅ Reentrancy guard + checks-effects-interactions |
| `finalize()` | ✅ Reentrancy guard + checks-effects-interactions |

---

## Token Assumptions

The raffle uses a minimal ERC20 interface (`IERC20Minimal`).

**Requirements:**
- Token must return `true` on successful `transfer()` and `transferFrom()`
- Token must not have fee-on-transfer mechanics
- Token must not be rebasing
- Token must have standard 6 or 18 decimals (USDC uses 6)

**Risk:** Non-compliant tokens may cause unexpected behavior or stuck funds.

---

## Known Limitations

| Limitation | Impact |
|------------|--------|
| No pause/emergency stop | Cannot halt operations in case of exploit |
| No on-chain randomness verification | Must trust oracle |
| No governance/multisig | Single admin key controls factory |
| No upgradeability | Raffle contracts are immutable |
| No fee-on-transfer support | Incompatible tokens will fail |

---

## Hardening Recommendations

### For Operators

1. **Secure Admin Keys**
   - Use a hardware wallet or multisig for the factory admin
   - Consider a timelock for admin actions

2. **Monitor Randomness Delivery**
   - Alert on failed or delayed randomness
   - Have a process for manual intervention

3. **Oracle Security**
   - Run redundant oracle infrastructure
   - Monitor for oracle downtime or misbehavior

4. **Token Verification**
   - Only use well-audited, standard ERC20 tokens
   - Verify token behavior before deployment

### For Deployment

1. **Test Thoroughly**
   - Run all tests before mainnet deployment
   - Use testnet for integration testing

2. **Verify Contracts**
   - Publish source code on block explorer
   - Consider third-party security audit

3. **Document Addresses**
   - Record all deployed contract addresses
   - Maintain deployment scripts in version control

---

## Threat Model Summary

| Threat | Mitigation |
|--------|------------|
| Malicious oracle | None (trust required) — future: on-chain verification |
| Admin key compromise | Use multisig + timelock |
| Reentrancy attacks | Reentrancy guards + CEI pattern |
| Front-running ticket purchases | N/A (ticket order doesn't affect winner) |
| Randomness manipulation | Off-chain Drand is publicly verifiable |
| Token drain | Access control on `finalize()` |
