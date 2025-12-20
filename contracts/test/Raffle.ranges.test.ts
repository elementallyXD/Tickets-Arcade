import { expect } from "chai";
import { network } from "hardhat";
import type { BaseContract } from "ethers";

type MockUSDCContract = BaseContract & {
  mint(to: string, amount: bigint): Promise<unknown>;
  approve(spender: string, amount: bigint): Promise<boolean>;
};

type RaffleContract = BaseContract & {
  buyTickets(count: number): Promise<unknown>;
  totalTickets(): Promise<bigint>;
  pot(): Promise<bigint>;
  ranges(index: number): Promise<{ buyer: string; start: bigint; end: bigint }>;
};

describe("Ticket Arcade - Raffle ticket ranges", function () {
  it("allocates contiguous ticket ranges and updates pot", async function () {
    const { ethers } = await network.connect();

    const [, alice, bob, feeRecipient] = await ethers.getSigners();

    const MockUSDC = await ethers.getContractFactory("MockUSDC");
    const usdc = (await MockUSDC.deploy()) as unknown as MockUSDCContract;
    await usdc.waitForDeployment();

    const MockRng = await ethers.getContractFactory("MockRandomnessProvider");
    const rng = await MockRng.deploy();
    await rng.waitForDeployment();

    const Factory = await ethers.getContractFactory("RaffleFactory");
    const maxFeeBps = 500;
    const factory = await Factory.deploy(await usdc.getAddress(), await rng.getAddress(), maxFeeBps);
    await factory.waitForDeployment();

    const block = await ethers.provider.getBlock("latest");
    const now = BigInt(block!.timestamp);

    const endTime = now + 3600n;
    const ticketPrice = 2_000_000n; // 2 USDC (6 decimals)
    const maxTickets = 100;
    const feeBps = 200;

    await factory.createRaffle(endTime, ticketPrice, maxTickets, feeBps, feeRecipient.address);

    const raffleAddr = await factory.raffles(0);
    const raffle = (await ethers.getContractAt("Raffle", raffleAddr)) as unknown as RaffleContract;

    await usdc.mint(alice.address, 1_000_000_000n);
    await usdc.mint(bob.address, 1_000_000_000n);

    const usdcAlice = usdc.connect(alice) as MockUSDCContract;
    const usdcBob = usdc.connect(bob) as MockUSDCContract;

    await usdcAlice.approve(raffleAddr, 1_000_000_000n);
    await usdcBob.approve(raffleAddr, 1_000_000_000n);

    const raffleAlice = raffle.connect(alice) as RaffleContract;
    const raffleBob = raffle.connect(bob) as RaffleContract;

    await raffleAlice.buyTickets(3); // [0..2]
    await raffleBob.buyTickets(2);   // [3..4]

    expect(await raffle.totalTickets()).to.equal(5n);
    expect(await raffle.pot()).to.equal(ticketPrice * 5n);

    const r0 = await raffle.ranges(0);
    expect(r0.buyer).to.equal(alice.address);
    expect(r0.start).to.equal(0n);
    expect(r0.end).to.equal(2n);

    const r1 = await raffle.ranges(1);
    expect(r1.buyer).to.equal(bob.address);
    expect(r1.start).to.equal(3n);
    expect(r1.end).to.equal(4n);
  });
});
