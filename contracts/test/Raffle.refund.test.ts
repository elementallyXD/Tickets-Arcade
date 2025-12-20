import { expect } from "chai";
import { network } from "hardhat";
import type { BaseContract } from "ethers";

type MockUSDCContract = BaseContract & {
  mint(to: string, amount: bigint): Promise<unknown>;
  approve(spender: string, amount: bigint): Promise<boolean>;
  balanceOf(owner: string): Promise<bigint>;
};

type RaffleContract = BaseContract & {
  buyTickets(count: number): Promise<unknown>;
  close(): Promise<unknown>;
  requestRandom(): Promise<unknown>;
  refund(): Promise<unknown>;
  refundsEnabled(): Promise<boolean>;
  REFUND_DELAY(): Promise<bigint>;
};

describe("Ticket Arcade - Refund flow", function () {
  it("allows refunds after delay if randomness never arrives", async function () {
    const { ethers } = await network.connect();
    const [deployer, alice, feeRecipient] = await ethers.getSigners();

    const MockUSDC = await ethers.getContractFactory("MockUSDC");
    const usdc = (await MockUSDC.deploy()) as unknown as MockUSDCContract;
    await usdc.waitForDeployment();

    const MockRng = await ethers.getContractFactory("MockRandomnessProvider");
    const rng = await MockRng.deploy();
    await rng.waitForDeployment();

    const Factory = await ethers.getContractFactory("RaffleFactory");
    const factory = await Factory.deploy(await usdc.getAddress(), await rng.getAddress(), 500);
    await factory.waitForDeployment();

    const block0 = await ethers.provider.getBlock("latest");
    const now = BigInt(block0!.timestamp);

    const ticketPrice = 1_000_000n;
    const maxTickets = 10;
    const feeBps = 200;
    const endTime = now + 3600n;

    await factory.createRaffle(endTime, ticketPrice, maxTickets, feeBps, feeRecipient.address);
    const raffleAddr = await factory.raffles(0);
    const raffle = (await ethers.getContractAt("Raffle", raffleAddr)) as unknown as RaffleContract;

    await usdc.mint(alice.address, 1_000_000_000n);
    const usdcAlice = usdc.connect(alice) as MockUSDCContract;
    const raffleAlice = raffle.connect(alice) as RaffleContract;

    await usdcAlice.approve(raffleAddr, 1_000_000_000n);

    await raffleAlice.buyTickets(4);

    // Move time past endTime to close
    await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
    await ethers.provider.send("evm_mine", []);
    await raffle.close();

    // Request randomness must be operator (creator=deployer). OK: raffle is connected to deployer by default.
    await raffle.requestRandom();

    // Jump past refund delay: endTime + 1 day + 1
    const refundDelay = await raffle.REFUND_DELAY();
    const refundAt = endTime + refundDelay;
    await ethers.provider.send("evm_setNextBlockTimestamp", [Number(refundAt + 1n)]);
    await ethers.provider.send("evm_mine", []);

    const before = await usdc.balanceOf(alice.address);
    await raffleAlice.refund();
    const after = await usdc.balanceOf(alice.address);

    expect(after - before).to.equal(ticketPrice * 4n);
    expect(await raffle.refundsEnabled()).to.equal(true);

    // Second refund should fail
    let reverted = false;
    try {
      await raffleAlice.refund();
    } catch (err) {
      reverted = true;
      expect(String(err)).to.include("AlreadyRefunded");
    }
    expect(reverted).to.equal(true);
  });
});
