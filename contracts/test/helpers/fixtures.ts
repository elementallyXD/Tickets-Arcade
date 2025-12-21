/**
 * Shared test fixtures and utilities for Raffle tests
 * 
 * This module provides common setup functions to reduce code duplication
 * across test files.
 */
import { network } from "hardhat";
import type { 
  MockUSDCContract, 
  MockRngContract, 
  RaffleContract,
  RaffleFactoryContract,
} from "./types.js";
import {
  DEFAULT_MINT_AMOUNT,
  DEFAULT_TICKET_PRICE,
  DEFAULT_MAX_TICKETS,
  DEFAULT_FEE_BPS,
  DEFAULT_MAX_FEE_BPS,
  ONE_HOUR,
} from "./types.js";

// =============================================================================
// FIXTURE TYPES
// =============================================================================

export interface TestContext {
  ethers: Awaited<ReturnType<typeof network.connect>>["ethers"];
  deployer: Awaited<ReturnType<typeof getSigners>>[0];
  alice: Awaited<ReturnType<typeof getSigners>>[1];
  bob: Awaited<ReturnType<typeof getSigners>>[2];
  charlie: Awaited<ReturnType<typeof getSigners>>[3];
  feeRecipient: Awaited<ReturnType<typeof getSigners>>[4];
  usdc: MockUSDCContract;
  rng: MockRngContract;
  factory: RaffleFactoryContract;
  raffle: RaffleContract;
  raffleAddr: string;
  endTime: bigint;
  ticketPrice: bigint;
  maxTickets: number;
  feeBps: number;
}

// Helper to get signers with proper typing
async function getSigners() {
  const { ethers } = await network.connect();
  return ethers.getSigners();
}

// =============================================================================
// FIXTURES
// =============================================================================

/**
 * Deploy all contracts and create a raffle ready for testing
 * 
 * Setup includes:
 * - MockUSDC deployed
 * - MockRandomnessProvider deployed  
 * - RaffleFactory deployed
 * - One raffle created (endTime = now + 1 hour)
 * - Alice, Bob, Charlie funded with USDC and approved
 */
export async function deployRaffleFixture(): Promise<TestContext> {
  const { ethers } = await network.connect();
  const [deployer, alice, bob, charlie, feeRecipient] = await ethers.getSigners();

  // Deploy MockUSDC
  const MockUSDC = await ethers.getContractFactory("MockUSDC");
  const usdc = (await MockUSDC.deploy()) as unknown as MockUSDCContract;
  await usdc.waitForDeployment();
  const usdcAddr = await usdc.getAddress();

  // Deploy MockRandomnessProvider
  const MockRng = await ethers.getContractFactory("MockRandomnessProvider");
  const rng = (await MockRng.deploy()) as unknown as MockRngContract;
  await rng.waitForDeployment();
  const rngAddr = await rng.getAddress();

  // Deploy Factory
  const Factory = await ethers.getContractFactory("RaffleFactory");
  const factory = (await Factory.deploy(
    usdcAddr, 
    rngAddr, 
    DEFAULT_MAX_FEE_BPS
  )) as unknown as RaffleFactoryContract;
  await factory.waitForDeployment();

  // Get current timestamp for endTime
  const latestBlock = await ethers.provider.getBlock("latest");
  const now = BigInt(latestBlock!.timestamp);
  const endTime = now + ONE_HOUR;

  // Create raffle
  await factory.createRaffle(
    endTime, 
    DEFAULT_TICKET_PRICE, 
    DEFAULT_MAX_TICKETS, 
    DEFAULT_FEE_BPS, 
    feeRecipient.address
  );
  
  const raffleAddr = await factory.raffles(0);
  const raffle = (await ethers.getContractAt("Raffle", raffleAddr)) as unknown as RaffleContract;

  // Fund test users with USDC
  await usdc.mint(alice.address, DEFAULT_MINT_AMOUNT);
  await usdc.mint(bob.address, DEFAULT_MINT_AMOUNT);
  await usdc.mint(charlie.address, DEFAULT_MINT_AMOUNT);

  // Approve raffle to spend USDC
  const usdcAlice = usdc.connect(alice) as MockUSDCContract;
  const usdcBob = usdc.connect(bob) as MockUSDCContract;
  const usdcCharlie = usdc.connect(charlie) as MockUSDCContract;

  await usdcAlice.approve(raffleAddr, DEFAULT_MINT_AMOUNT);
  await usdcBob.approve(raffleAddr, DEFAULT_MINT_AMOUNT);
  await usdcCharlie.approve(raffleAddr, DEFAULT_MINT_AMOUNT);

  return {
    ethers,
    deployer,
    alice,
    bob,
    charlie,
    feeRecipient,
    usdc,
    rng,
    factory,
    raffle,
    raffleAddr,
    endTime,
    ticketPrice: DEFAULT_TICKET_PRICE,
    maxTickets: DEFAULT_MAX_TICKETS,
    feeBps: DEFAULT_FEE_BPS,
  };
}

// =============================================================================
// TIME HELPERS
// =============================================================================

/**
 * Advance blockchain time to a specific timestamp
 */
export async function advanceTimeTo(ethers: TestContext["ethers"], timestamp: bigint): Promise<void> {
  await ethers.provider.send("evm_setNextBlockTimestamp", [Number(timestamp)]);
  await ethers.provider.send("evm_mine", []);
}

/**
 * Advance blockchain time past the raffle endTime
 */
export async function advancePastEndTime(ctx: TestContext): Promise<void> {
  await advanceTimeTo(ctx.ethers, ctx.endTime + 1n);
}

/**
 * Advance blockchain time past the refund delay
 */
export async function advancePastRefundDelay(ctx: TestContext): Promise<void> {
  const refundAt = await ctx.raffle.refundAvailableAt();
  await advanceTimeTo(ctx.ethers, refundAt + 1n);
}

// =============================================================================
// RAFFLE LIFECYCLE HELPERS
// =============================================================================

/**
 * Complete the raffle lifecycle up to CLOSED status
 * Optionally buys tickets before closing
 */
export async function closeRaffle(
  ctx: TestContext, 
  ticketsBefore?: { buyer: RaffleContract; count: number }[]
): Promise<void> {
  // Buy tickets if specified
  if (ticketsBefore) {
    for (const { buyer, count } of ticketsBefore) {
      await buyer.buyTickets(count);
    }
  }
  
  // Advance time and close
  await advancePastEndTime(ctx);
  await ctx.raffle.close();
}

/**
 * Complete the raffle through randomness fulfillment
 */
export async function fulfillRandomness(
  ctx: TestContext,
  randomnessValue: bigint = 12345n
): Promise<void> {
  await ctx.raffle.requestRandom();
  const reqId = await ctx.raffle.requestId();
  await ctx.rng.fulfill(reqId, randomnessValue);
}

/**
 * Run complete raffle from purchase to finalization
 */
export async function runCompleteRaffle(
  ctx: TestContext,
  tickets: { buyer: RaffleContract; count: number }[],
  randomnessValue: bigint = 12345n
): Promise<void> {
  // Buy tickets
  for (const { buyer, count } of tickets) {
    await buyer.buyTickets(count);
  }
  
  // Close and finalize
  await advancePastEndTime(ctx);
  await ctx.raffle.close();
  await fulfillRandomness(ctx, randomnessValue);
  await ctx.raffle.finalize();
}

// =============================================================================
// ASSERTION HELPERS
// =============================================================================

/**
 * Expect a transaction to revert with a specific error
 */
export async function expectRevert(
  promise: Promise<unknown>,
  errorName: string
): Promise<boolean> {
  try {
    await promise;
    return false; // Should have reverted
  } catch (err) {
    const errorStr = String(err);
    if (!errorStr.includes(errorName)) {
      throw new Error(`Expected error "${errorName}" but got: ${errorStr}`);
    }
    return true;
  }
}
