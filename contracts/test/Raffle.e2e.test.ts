import { expect } from "chai";
import { network } from "hardhat";
import type { MockRngContract, MockUSDCContract, RaffleContract } from "./helpers/types.js";

describe("Ticket Arcade - Raffle E2E flow", function () {
  it("closes, requests randomness, fulfills, finalizes, and pays winner + fee", async function () {
    const { ethers } = await network.connect();
    const [, alice, bob, feeRecipient] = await ethers.getSigners();

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
    const maxFeeBps = 500; // 5%
    const factory = await Factory.deploy(usdcAddr, rngAddr, maxFeeBps);
    await factory.waitForDeployment();

    // Create raffle (short endTime so we can close via time travel)
    const latestBlock = await ethers.provider.getBlock("latest");
    const now = BigInt(latestBlock!.timestamp);

    const ticketPrice = 1_000_000n; // 1 USDC (6 decimals)
    const maxTickets = 10;
    const feeBps = 200; // 2%
    const endTime = now + 3600n; // +1 hour

    await factory.createRaffle(endTime, ticketPrice, maxTickets, feeBps, feeRecipient.address);

    const raffleAddr = await factory.raffles(0);
    const raffle = (await ethers.getContractAt("Raffle", raffleAddr)) as unknown as RaffleContract;

    // Fund buyers + approvals
    const mintAmount = 1_000_000_000n;
    await usdc.mint(alice.address, mintAmount);
    await usdc.mint(bob.address, mintAmount);

    const usdcAlice = usdc.connect(alice) as MockUSDCContract;
    const usdcBob = usdc.connect(bob) as MockUSDCContract;

    await usdcAlice.approve(raffleAddr, mintAmount);
    await usdcBob.approve(raffleAddr, mintAmount);

    // Buy tickets:
    // Alice buys 3 -> indices 0..2
    // Bob buys 2 -> indices 3..4
    const raffleAlice = raffle.connect(alice) as RaffleContract;
    const raffleBob = raffle.connect(bob) as RaffleContract;

    await raffleAlice.buyTickets(3);
    await raffleBob.buyTickets(2);

    expect(await raffle.totalTickets()).to.equal(5n);
    expect(await raffle.pot()).to.equal(ticketPrice * 5n);

    // Time travel past endTime so close() is allowed
    await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
    await ethers.provider.send("evm_mine", []);

    // Close raffle
    await raffle.close();

    // Request randomness
    await raffle.requestRandom();
    const reqId = await raffle.requestId();
    expect(reqId).to.not.equal(0n);

    // Choose deterministic randomness so we can predict winner
    // winningIndex = randomness % totalTickets
    // Use randomness = 3 => winningIndex=3 => belongs to Bob (3..4)
    const randomness = 3n;

    await rng.fulfill(reqId, randomness);

    expect(await raffle.randomness()).to.equal(randomness);
    expect(await raffle.winningIndex()).to.equal(3n);

    // Capture balances before finalize
    const pot = await raffle.pot();
    const feeAmount = (pot * BigInt(feeBps)) / 10000n;
    const prizeAmount = pot - feeAmount;

    const bobBefore = await usdc.balanceOf(bob.address);
    const feeBefore = await usdc.balanceOf(feeRecipient.address);

    // Finalize
    await raffle.finalize();

    // Winner should be Bob
    expect(await raffle.winner()).to.equal(bob.address);

    // Status should be FINALIZED (enum index 4)
    expect(await raffle.status()).to.equal(4n);

    // Assert payouts
    const bobAfter = await usdc.balanceOf(bob.address);
    const feeAfter = await usdc.balanceOf(feeRecipient.address);

    expect(bobAfter - bobBefore).to.equal(prizeAmount);
    expect(feeAfter - feeBefore).to.equal(feeAmount);
  });
});
