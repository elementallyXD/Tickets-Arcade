# Ticket Arcade - Security Review

**Reviewer:** Senior Smart Contract Security Engineer  
**Date:** December 21, 2025  
**Commit:** Initial Review  
**Scope:** `Raffle.sol`, `RaffleFactory.sol`, `IRandomnessProvider.sol`, `IERC20Minimal.sol`

---

## Executive Summary

This review covers the Ticket Arcade raffle system MVP. The contracts implement a raffle where users buy tickets with USDC, randomness is requested from a VRF provider, and a winner is selected and paid out with fees to a designated recipient.

Overall, the codebase follows reasonable security practices but has several issues that need addressing before production deployment.

---

## Threat Model & Assumptions

1. **USDC Token**: Assumed to be a standard, non-rebasing, non-fee-on-transfer ERC20 token.
2. **Randomness Provider**: Trusted to eventually fulfill requests with valid randomness. In production, this would be a VRF like Chainlink.
3. **Creator/Keeper**: Semi-trusted operators who manage raffle lifecycle.
4. **Users**: Untrusted; may attempt to exploit contract logic.
5. **No Admin Keys for Raffle**: Once deployed, creator cannot drain funds (by design).

---

## Findings

### CRITICAL

**None found.**

---

### HIGH

#### H-1: Finalize Writes State After External Calls (CEI Violation)

**Location:** [Raffle.sol](contracts/Raffle.sol) - `finalize()` function

**Description:**  
The `finalize()` function sets `status = Status.FINALIZED` *after* the ERC20 transfer calls. While USDC is assumed safe, if the token were malicious or had hooks (ERC-777 style), this could allow reentrancy that bypasses the `AlreadyFinalized` check.

The current code does:
```solidity
if (prizeAmount > 0) {
    bool ok1 = usdc.transfer(winnerAddress, prizeAmount);
    if (!ok1) revert InvalidRequest();
}
if (feeAmount > 0) {
    bool ok2 = usdc.transfer(feeRecipient, feeAmount);
    if (!ok2) revert InvalidRequest();
}
status = Status.FINALIZED;  // <-- After interactions
```

**Impact:** Potential double-payout via reentrancy if token has transfer hooks.

**Recommendation:** Move `status = Status.FINALIZED` before external calls. Also set `pot = 0` before transfers.

**Fix:**
```solidity
status = Status.FINALIZED;
pot = 0;  // Clear pot before transfers
// then do transfers...
```

---

#### H-2: Winner Check Uses `address(0)` Instead of Status

**Location:** [Raffle.sol](contracts/Raffle.sol#L326) - `finalize()` function

**Description:**  
The function checks `if (winner != address(0)) revert AlreadyFinalized();` but this is redundant with the status check. More importantly, if `_findWinner` returns `address(0)` (which it can if ranges are somehow empty/corrupted), the winner would be set to `address(0)` and funds would be sent to the zero address, losing them forever.

**Impact:** Potential permanent loss of funds.

**Recommendation:** Add explicit check that `_findWinner` returns a non-zero address.

**Fix:**
```solidity
address winnerAddress = _findWinner(uint32(winningIndex));
if (winnerAddress == address(0)) revert InvalidRequest(); // Safety check
winner = winnerAddress;
```

---

### MEDIUM

#### M-1: No Check for Zero Tickets Before Close

**Location:** [Raffle.sol](contracts/Raffle.sol) - `_close()` function

**Description:**  
A raffle can be closed with zero tickets sold. While `requestRandom()` does check for `totalTickets == 0`, the state machine allows a raffle to sit in `CLOSED` status forever with zero tickets, blocking refund paths (since there's nothing to refund).

**Impact:** Stuck state with no resolution path for zero-ticket raffles.

**Recommendation:** Allow raffles with zero tickets to be "finalized" in a cancelled state, or prevent closing with zero tickets and add explicit cancellation.

**Fix:** For MVP simplicity, the current behavior is acceptable but should be documented. Alternatively, add:
```solidity
// In requestRandom(), the NoTickets check is sufficient
// Document: Zero-ticket raffles remain in CLOSED forever (no funds at risk)
```

---

#### M-2: `close()` Can Be Called by Factory Indefinitely

**Location:** [Raffle.sol](contracts/Raffle.sol#L226) - `close()` with `onlyOperatorOrFactory`

**Description:**  
The factory can call `close()` on any raffle, but there's no reason for the factory to do so after deployment. This expands the attack surface if the factory were compromised.

**Impact:** Low severity but unnecessary privilege.

**Recommendation:** Remove `factory` from `onlyOperatorOrFactory` modifier for `close()`. The operator (creator/keeper) should be sufficient.

**Fix:** Change modifier on `close()` from `onlyOperatorOrFactory` to `onlyOperator`, OR make `close()` permissionless after conditions are met (time passed or sold out).

---

#### M-3: Missing Event for Refunds-Enabled State Change

**Location:** [Raffle.sol](contracts/Raffle.sol) - `refund()` function

**Description:**  
When `refundsEnabled` is set to `true` for the first time, no dedicated event is emitted. The backend relies on events for indexing, so this state change could be missed.

**Impact:** Backend indexer may miss critical state transition.

**Recommendation:** Emit a dedicated event when `refundsEnabled` becomes true.

**Fix:**
```solidity
event RefundsStarted(uint256 indexed raffleId, uint256 timestamp);

// In refund():
if (!refundsEnabled) {
    refundsEnabled = true;
    emit RefundsStarted(raffleId, block.timestamp);
}
```

---

### LOW

#### L-1: Duplicate Status Check in `finalize()`

**Location:** [Raffle.sol](contracts/Raffle.sol) - `finalize()` function

**Description:**  
The function checks both `status != Status.RANDOM_FULFILLED` and `winner != address(0)`. If status is RANDOM_FULFILLED and winner is set, status must be FINALIZED (since winner is only set in finalize). The second check is redundant.

**Impact:** Wasted gas, code complexity.

**Recommendation:** Remove the `winner != address(0)` check; rely solely on status.

---

#### L-2: `buyTickets()` Does Not Validate Against `uint32` Overflow for Very Large Counts

**Location:** [Raffle.sol](contracts/Raffle.sol) - `buyTickets()` function

**Description:**  
While `maxTickets` is `uint32` and limits total tickets, the `count` parameter is also `uint32`. The current logic safely handles this because:
- `totalTickets + count > maxTickets` check prevents overflow in ticket indices
- `endIndex = startIndex + count - 1` is safe because count >= 1

However, `ticketsByBuyer[msg.sender] += count` could theoretically overflow if a user bought more than `uint32.max` tickets across multiple purchases. This is practically impossible given reasonable `maxTickets` limits.

**Impact:** Negligible; theoretical edge case.

**Recommendation:** No action needed for MVP. Document assumption that `maxTickets <= 2^32 - 1`.

---

#### L-3: Events Emitted in Wrong Order in `finalize()`

**Location:** [Raffle.sol](contracts/Raffle.sol) - `finalize()` function

**Description:**  
`PayoutsCompleted` is emitted before `WinnerSelected`. Logically, winner selection happens before payout.

**Impact:** Minor confusion for event consumers.

**Recommendation:** Emit `WinnerSelected` first, then `PayoutsCompleted`.

---

#### L-4: `locked` Reentrancy Guard Declared After Functions Using It

**Location:** [Raffle.sol](contracts/Raffle.sol#L109)

**Description:**  
The `locked` state variable is declared after the error definitions but before the modifiers that use it. This is fine functionally but reduces code readability.

**Impact:** Code clarity only.

**Recommendation:** Move `locked` to the "Mutable state" section.

---

### INFORMATIONAL

#### I-1: Consider Making `close()` Fully Permissionless

**Description:**  
Currently `close()` requires operator permission. Since the conditions for closing (time passed OR sold out) are objective, making it permissionless would allow anyone to trigger the state transition, improving decentralization.

**Recommendation:** Remove access control from `close()`.

---

#### I-2: Missing NatSpec Documentation

**Description:**  
Functions lack NatSpec comments (`@notice`, `@param`, `@return`).

**Recommendation:** Add NatSpec for all public/external functions.

---

#### I-3: Consider Emitting More Detailed Refund Event

**Description:**  
`RefundClaimed` could include `ticketCount` for easier indexing.

**Recommendation:**
```solidity
event RefundClaimed(uint256 indexed raffleId, address indexed buyer, uint32 ticketCount, uint256 amount);
```

---

#### I-4: RaffleFactory Has No Mechanism to Pause Creation

**Description:**  
If a critical bug is found, there's no way to pause new raffle creation.

**Recommendation:** Consider adding a paused flag (optional for MVP).

---

#### I-5: No Upper Bound Check on `endTime` Duration

**Description:**  
A creator could set `endTime` years in the future, creating raffles that never close.

**Recommendation:** Consider adding a maximum duration (e.g., 30 days).

---

## State Machine Analysis

```
ACTIVE -> CLOSED -> RANDOM_REQUESTED -> RANDOM_FULFILLED -> FINALIZED
                  \                    \
                   \                    -> (refund path if delay passed)
                    -> (refund path if delay passed)
```

**Valid Transitions:**
- `ACTIVE` → `CLOSED`: via `close()` or auto-close on sold out
- `CLOSED` → `RANDOM_REQUESTED`: via `requestRandom()`
- `RANDOM_REQUESTED` → `RANDOM_FULFILLED`: via `fulfillRandomness()`
- `RANDOM_FULFILLED` → `FINALIZED`: via `finalize()`

**Refund Path:**
- From `CLOSED` or `RANDOM_REQUESTED`: If `endTime + REFUND_DELAY` has passed
- Once any refund occurs, `refundsEnabled = true` blocks `fulfillRandomness()` and `finalize()`

**Edge Cases Verified:**
- ✅ Cannot buy after closed
- ✅ Cannot close twice (reverts `NotActive`)
- ✅ Cannot request random before close
- ✅ Cannot finalize before fulfill
- ✅ Cannot refund after finalize (status check)
- ✅ Cannot fulfill after refunds started
- ✅ Cannot double-refund (mapping check)

---

## Recommendations Summary

| ID | Severity | Issue | Action | Status |
|----|----------|-------|--------|--------|
| H-1 | High | CEI violation in finalize() | Fix: Move status update before transfers | ✅ Fixed |
| H-2 | High | No check for zero-address winner | Fix: Add explicit check | ✅ Fixed |
| M-1 | Medium | Zero-ticket raffle edge case | Document or add cancellation | ✅ Documented |
| M-2 | Medium | Factory can close any raffle | Fix: Made close() permissionless | ✅ Fixed |
| M-3 | Medium | Missing RefundsStarted event | Fix: Add event | ✅ Fixed |
| L-1 | Low | Duplicate check in finalize | Fix: Remove redundant check | ✅ Fixed |
| L-3 | Low | Event order in finalize | Fix: Reorder events | ✅ Fixed |
| L-4 | Low | Variable ordering | Fix: Reorganize declarations | ✅ Fixed |
| I-1 | Info | close() access control | Made permissionless | ✅ Implemented |
| I-2 | Info | Missing NatSpec | Add documentation | ✅ Added |
| I-3 | Info | RefundClaimed detail | Add ticketCount | ✅ Fixed |

---

## Post-Fix Verification Checklist

- [x] All tests pass (30 tests passing)
- [x] Compile without warnings
- [x] CEI pattern followed in all fund-moving functions
- [x] Events emitted for all state changes
- [x] No reentrancy vulnerabilities
- [x] Access control correct
- [x] Edge cases handled

---

## Changes Made

### Raffle.sol
1. **CEI Pattern in `finalize()`**: Status and pot updates now happen before ERC20 transfers
2. **CEI Pattern in `refund()`**: State updates before ERC20 transfer
3. **Zero-address winner check**: Added `WinnerNotFound` error and explicit check
4. **`close()` permissionless**: Anyone can close after conditions met
5. **RefundsStarted event**: New event emitted on first refund
6. **RefundClaimed enhanced**: Now includes `ticketCount`
7. **Event ordering**: `WinnerSelected` now emitted before `PayoutsCompleted`
8. **Variable organization**: Reorganized with clear section headers
9. **NatSpec comments**: Added comprehensive documentation for all functions
10. **Removed redundant check**: `winner != address(0)` check removed (status is sufficient)
11. **Error naming**: Renamed errors for clarity (`InvalidCount` → `InvalidTicketCount`, `Reentrancy` → `ReentrancyGuard`, `RefundsEnabled` → `RefundsAlreadyEnabled`)

### RaffleFactory.sol
1. **NatSpec comments**: Added comprehensive documentation

### Tests Added
- [Raffle.security.test.ts](test/Raffle.security.test.ts): 27 security edge case tests covering:
  - State machine correctness (4 tests)
  - Refund path blocking and events (4 tests)
  - Access control verification (5 tests)
  - Randomness security (2 tests)
  - Edge cases (12 tests):
    - Auto-close on sold out
    - Zero tickets handling
    - Winner selection via binary search
    - `canRefund()` and `refundAmount()` view functions
    - Buying zero/excess tickets
    - Double refund prevention
    - Refund amount transfer verification
    - RefundClaimed event data verification

---

## Appendix: Gas Considerations

The `_findWinner()` function uses binary search (O(log n)) which is efficient. For MVP with reasonable range counts (< 1000), this is acceptable.

---

*End of Security Review*
