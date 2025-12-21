import type { BaseContract } from "ethers";

export type MockUSDCContract = BaseContract & {
  mint(to: string, amount: bigint): Promise<unknown>;
  approve(spender: string, amount: bigint): Promise<boolean>;
  balanceOf(owner: string): Promise<bigint>;
};

export type RaffleContract = BaseContract & {
  buyTickets(count: number): Promise<unknown>;
  totalTickets(): Promise<bigint>;
  pot(): Promise<bigint>;
  close(): Promise<unknown>;
  requestRandom(): Promise<unknown>;
  requestId(): Promise<bigint>;
  randomness(): Promise<bigint>;
  winningIndex(): Promise<bigint>;
  finalize(): Promise<unknown>;
  winner(): Promise<string>;
  status(): Promise<bigint>;
  ranges(index: number): Promise<{ buyer: string; start: bigint; end: bigint }>;
  refund(): Promise<unknown>;
  refundsEnabled(): Promise<boolean>;
  refundAvailableAt(): Promise<bigint>;
};

export type MockRngContract = BaseContract & {
  fulfill(requestId: bigint, randomness: bigint): Promise<unknown>;
};
