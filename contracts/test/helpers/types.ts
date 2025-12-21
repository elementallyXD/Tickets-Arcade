/**
 * Type definitions for test contracts
 * These types provide TypeScript type safety for contract interactions in tests
 */
import type { BaseContract, ContractTransactionResponse } from "ethers";

// =============================================================================
// STATUS VALUES (mirrors Raffle.Status)
// =============================================================================

export const RaffleStatus = {
  ACTIVE: 0n,
  CLOSED: 1n,
  RANDOM_REQUESTED: 2n,
  RANDOM_FULFILLED: 3n,
  FINALIZED: 4n,
} as const;

export type RaffleStatus = typeof RaffleStatus[keyof typeof RaffleStatus];

// =============================================================================
// CONTRACT TYPES
// =============================================================================

/**
 * MockUSDC test token contract
 */
export type MockUSDCContract = BaseContract & {
  mint(to: string, amount: bigint): Promise<ContractTransactionResponse>;
  approve(spender: string, amount: bigint): Promise<ContractTransactionResponse>;
  balanceOf(owner: string): Promise<bigint>;
  allowance(owner: string, spender: string): Promise<bigint>;
  transfer(to: string, amount: bigint): Promise<ContractTransactionResponse>;
  transferFrom(from: string, to: string, amount: bigint): Promise<ContractTransactionResponse>;
  decimals(): Promise<bigint>;
};

/**
 * Raffle contract - main raffle logic
 */
export type RaffleContract = BaseContract & {
  // View functions
  raffleId(): Promise<bigint>;
  status(): Promise<bigint>;
  totalTickets(): Promise<bigint>;
  pot(): Promise<bigint>;
  requestId(): Promise<bigint>;
  randomness(): Promise<bigint>;
  winningIndex(): Promise<bigint>;
  winner(): Promise<string>;
  creator(): Promise<string>;
  keeper(): Promise<string>;
  endTime(): Promise<bigint>;
  ticketPrice(): Promise<bigint>;
  maxTickets(): Promise<bigint>;
  feeBps(): Promise<bigint>;
  feeRecipient(): Promise<string>;
  refundsEnabled(): Promise<boolean>;
  refundAvailableAt(): Promise<bigint>;
  rangesCount(): Promise<bigint>;
  canRefund(buyer: string): Promise<boolean>;
  refundAmount(buyer: string): Promise<bigint>;
  ticketsByBuyer(buyer: string): Promise<bigint>;
  refunded(buyer: string): Promise<boolean>;
  ranges(index: number): Promise<{ buyer: string; start: bigint; end: bigint }>;
  
  // State-changing functions
  buyTickets(count: number): Promise<ContractTransactionResponse>;
  close(): Promise<ContractTransactionResponse>;
  requestRandom(): Promise<ContractTransactionResponse>;
  finalize(): Promise<ContractTransactionResponse>;
  refund(): Promise<ContractTransactionResponse>;
  setKeeper(newKeeper: string): Promise<ContractTransactionResponse>;
};

/**
 * RaffleFactory contract - deploys raffles
 */
export type RaffleFactoryContract = BaseContract & {
  // View functions
  admin(): Promise<string>;
  usdc(): Promise<string>;
  randomnessProvider(): Promise<string>;
  maxFeeBps(): Promise<bigint>;
  nextRaffleId(): Promise<bigint>;
  rafflesCount(): Promise<bigint>;
  raffles(index: number): Promise<string>;
  
  // State-changing functions
  createRaffle(
    endTime: bigint,
    ticketPrice: bigint,
    maxTickets: number,
    feeBps: number,
    feeRecipient: string
  ): Promise<ContractTransactionResponse>;
  setRandomnessProvider(newProvider: string): Promise<ContractTransactionResponse>;
  applyRandomnessProvider(): Promise<ContractTransactionResponse>;
  setMaxFeeBps(newMaxFeeBps: number): Promise<ContractTransactionResponse>;
};

/**
 * MockRandomnessProvider - test VRF provider
 */
export type MockRngContract = BaseContract & {
  nextRequestId(): Promise<bigint>;
  requestToRaffle(requestId: bigint): Promise<string>;
  requestRandomness(raffleId: bigint): Promise<ContractTransactionResponse>;
  fulfill(requestId: bigint, randomness: bigint): Promise<ContractTransactionResponse>;
};

// =============================================================================
// TEST CONSTANTS
// =============================================================================

/** Standard mint amount for test users (100 USDC) */
export const DEFAULT_MINT_AMOUNT = 100_000_000n;

/** Standard ticket price (1 USDC) */
export const DEFAULT_TICKET_PRICE = 1_000_000n;

/** Standard max tickets for tests */
export const DEFAULT_MAX_TICKETS = 10;

/** Standard fee (2% = 200 bps) */
export const DEFAULT_FEE_BPS = 200;

/** Max factory fee (5% = 500 bps) */
export const DEFAULT_MAX_FEE_BPS = 500;

/** One hour in seconds */
export const ONE_HOUR = 3600n;

/** One day in seconds (matches REFUND_DELAY) */
export const ONE_DAY = 86400n;
