import { expect } from "chai";
import { network } from "hardhat";
import type { MockRngContract, MockUSDCContract, RaffleContract } from "./helpers/types.js";

/**
 * Security regression tests for Raffle contract
 * Tests edge cases and security fixes identified in SECURITY_REVIEW.md
 */
describe("Ticket Arcade - Security Edge Cases", function () {
  
  async function deployRaffleFixture() {
    const { ethers } = await network.connect();
    const [deployer, alice, bob, charlie, feeRecipient] = await ethers.getSigners();

    const MockUSDC = await ethers.getContractFactory("MockUSDC");
    const usdc = (await MockUSDC.deploy()) as unknown as MockUSDCContract;
    await usdc.waitForDeployment();
    const usdcAddr = await usdc.getAddress();

    const MockRng = await ethers.getContractFactory("MockRandomnessProvider");
    const rng = (await MockRng.deploy()) as unknown as MockRngContract;
    await rng.waitForDeployment();
    const rngAddr = await rng.getAddress();

    const Factory = await ethers.getContractFactory("RaffleFactory");
    const maxFeeBps = 500;
    const factory = await Factory.deploy(usdcAddr, rngAddr, maxFeeBps);
    await factory.waitForDeployment();

    const latestBlock = await ethers.provider.getBlock("latest");
    const now = BigInt(latestBlock!.timestamp);
    const endTime = now + 3600n;
    const ticketPrice = 1_000_000n;
    const maxTickets = 10;
    const feeBps = 200;

    await factory.createRaffle(endTime, ticketPrice, maxTickets, feeBps, feeRecipient.address);
    const raffleAddr = await factory.raffles(0);
    const raffle = (await ethers.getContractAt("Raffle", raffleAddr)) as unknown as RaffleContract;

    // Fund users
    const mintAmount = 100_000_000n;
    await usdc.mint(alice.address, mintAmount);
    await usdc.mint(bob.address, mintAmount);
    await usdc.mint(charlie.address, mintAmount);

    const usdcAlice = usdc.connect(alice) as MockUSDCContract;
    const usdcBob = usdc.connect(bob) as MockUSDCContract;
    const usdcCharlie = usdc.connect(charlie) as MockUSDCContract;

    await usdcAlice.approve(raffleAddr, mintAmount);
    await usdcBob.approve(raffleAddr, mintAmount);
    await usdcCharlie.approve(raffleAddr, mintAmount);

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
      endTime,
      ticketPrice,
      maxTickets,
      feeBps,
      raffleAddr,
    };
  }

  describe("State Machine Correctness", function () {
    it("should not allow close() twice", async function () {
      const { raffle, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      
      // Second close should fail
      let reverted = false;
      try {
        await raffle.close();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("NotActive");
      }
      expect(reverted).to.equal(true);
    });

    it("should not allow requestRandom() before close", async function () {
      const { raffle, alice } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      let reverted = false;
      try {
        await raffle.requestRandom();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("NotClosed");
      }
      expect(reverted).to.equal(true);
    });

    it("should not allow finalize() before fulfill", async function () {
      const { raffle, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      await raffle.requestRandom();
      
      let reverted = false;
      try {
        await raffle.finalize();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("NotRandomFulfilled");
      }
      expect(reverted).to.equal(true);
    });

    it("should not allow buyTickets() after close", async function () {
      const { raffle, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      
      let reverted = false;
      try {
        await raffleAlice.buyTickets(1);
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("NotActive");
      }
      expect(reverted).to.equal(true);
    });
  });

  describe("Refund Path", function () {
    it("should not allow refund after finalize", async function () {
      const { raffle, rng, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(3);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      await raffle.requestRandom();
      
      const reqId = await raffle.requestId();
      await rng.fulfill(reqId, 12345n);
      
      await raffle.finalize();
      
      // Now try to refund - should fail because status is FINALIZED
      let reverted = false;
      try {
        await raffleAlice.refund();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("RefundsNotAvailable");
      }
      expect(reverted).to.equal(true);
    });

    it("should block fulfill after refundsEnabled", async function () {
      const { raffle, rng, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(3);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      await raffle.requestRandom();
      
      const reqId = await raffle.requestId();
      
      // Jump past refund delay
      const refundAt = await raffle.refundAvailableAt();
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(refundAt + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      // Alice refunds
      await raffleAlice.refund();
      expect(await raffle.refundsEnabled()).to.equal(true);
      
      // Now fulfill should fail
      let reverted = false;
      try {
        await rng.fulfill(reqId, 12345n);
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("RefundsAlreadyEnabled");
      }
      expect(reverted).to.equal(true);
    });

    it("should block finalize after refundsEnabled (refund in RANDOM_REQUESTED)", async function () {
      const { raffle, rng, alice, bob, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      const raffleBob = raffle.connect(bob) as RaffleContract;
      await raffleAlice.buyTickets(3);
      await raffleBob.buyTickets(2);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      await raffle.requestRandom();
      
      const reqId = await raffle.requestId();
      
      // Jump past refund delay (BEFORE fulfill)
      const refundAt = await raffle.refundAvailableAt();
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(refundAt + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      // Alice refunds - this sets refundsEnabled
      await raffleAlice.refund();
      expect(await raffle.refundsEnabled()).to.equal(true);
      
      // Now try to fulfill - should fail because refundsEnabled
      let fulfillReverted = false;
      try {
        await rng.fulfill(reqId, 12345n);
      } catch (err) {
        fulfillReverted = true;
        expect(String(err)).to.include("RefundsAlreadyEnabled");
      }
      expect(fulfillReverted).to.equal(true);
      
      // Status is still RANDOM_REQUESTED, finalize should fail
      // Either NotRandomFulfilled (status check) or RefundsAlreadyEnabled (refunds check) 
      let finalizeReverted = false;
      try {
        await raffle.finalize();
      } catch (err) {
        finalizeReverted = true;
        // Could be either RefundsAlreadyEnabled or NotRandomFulfilled depending on check order
        const errorStr = String(err);
        expect(errorStr.includes("NotRandomFulfilled") || errorStr.includes("RefundsAlreadyEnabled")).to.equal(true);
      }
      expect(finalizeReverted).to.equal(true);
      
      // Bob can also refund
      await raffleBob.refund();
      expect(await raffle.pot()).to.equal(0n);
    });

    it("should emit RefundsStarted event on first refund", async function () {
      const { raffle, alice, endTime, ethers, raffleAddr } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(3);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      await raffle.requestRandom();
      
      const refundAt = await raffle.refundAvailableAt();
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(refundAt + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      // Check for RefundsStarted event
      const raffleContract = await ethers.getContractAt("Raffle", raffleAddr);
      const tx = await raffleAlice.refund();
      const receipt = await (tx as any).wait();
      
      // Find RefundsStarted event
      const iface = raffleContract.interface;
      const refundsStartedTopic = iface.getEvent("RefundsStarted")!.topicHash;
      const hasRefundsStarted = receipt.logs.some(
        (log: any) => log.topics[0] === refundsStartedTopic
      );
      expect(hasRefundsStarted).to.equal(true);
    });
  });

  describe("Access Control", function () {
    it("should allow anyone to close() after endTime", async function () {
      const { raffle, alice, charlie, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      // Charlie (random user, not operator) can close
      const raffleCharlie = raffle.connect(charlie) as RaffleContract;
      await raffleCharlie.close();
      
      expect(await raffle.status()).to.equal(1n); // CLOSED
    });

    it("should not allow non-operator to call requestRandom()", async function () {
      const { raffle, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffleAlice.close();
      
      // Alice (non-operator) cannot request random
      let reverted = false;
      try {
        await raffleAlice.requestRandom();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("Unauthorized");
      }
      expect(reverted).to.equal(true);
    });

    it("should not allow non-operator to call finalize()", async function () {
      const { raffle, rng, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      await raffle.requestRandom();
      
      const reqId = await raffle.requestId();
      await rng.fulfill(reqId, 12345n);
      
      // Alice (non-operator) cannot finalize
      let reverted = false;
      try {
        await raffleAlice.finalize();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("Unauthorized");
      }
      expect(reverted).to.equal(true);
    });

    it("should allow keeper to operate", async function () {
      const { raffle, rng, alice, bob, endTime, ethers, deployer } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      // Set bob as keeper (deployer is creator)
      await (raffle.connect(deployer) as RaffleContract).setKeeper(bob.address);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      
      // Bob (keeper) can request random
      const raffleBob = raffle.connect(bob) as RaffleContract;
      await raffleBob.requestRandom();
      
      const reqId = await raffle.requestId();
      await rng.fulfill(reqId, 12345n);
      
      // Bob can finalize
      await raffleBob.finalize();
      expect(await raffle.status()).to.equal(4n); // FINALIZED
    });

    it("should only allow creator to set keeper", async function () {
      const { raffle, alice, bob } = await deployRaffleFixture();
      
      // Alice (non-creator) cannot set keeper
      let reverted = false;
      try {
        await (raffle.connect(alice) as RaffleContract).setKeeper(bob.address);
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("Unauthorized");
      }
      expect(reverted).to.equal(true);
    });
  });

  describe("Randomness Security", function () {
    it("should reject wrong requestId", async function () {
      const { raffle, rng, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      await raffle.requestRandom();
      
      const reqId = await raffle.requestId();
      const wrongReqId = reqId + 999n;
      
      let reverted = false;
      try {
        await rng.fulfill(wrongReqId, 12345n);
      } catch (err) {
        reverted = true;
        // MockRandomnessProvider will revert with "unknown requestId"
        expect(String(err)).to.include("unknown requestId");
      }
      expect(reverted).to.equal(true);
    });

    it("should reject zero randomness", async function () {
      const { raffle, rng, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      await raffle.requestRandom();
      
      const reqId = await raffle.requestId();
      
      let reverted = false;
      try {
        await rng.fulfill(reqId, 0n);
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("randomness=0");
      }
      expect(reverted).to.equal(true);
    });
  });

  describe("Edge Cases", function () {
    it("should auto-close on sold out and allow finalization", async function () {
      const { raffle, rng, alice, bob, charlie, ethers, maxTickets, ticketPrice, usdc, feeRecipient, feeBps } = 
        await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      const raffleBob = raffle.connect(bob) as RaffleContract;
      const raffleCharlie = raffle.connect(charlie) as RaffleContract;
      
      // Buy all tickets to trigger auto-close
      await raffleAlice.buyTickets(4);
      await raffleBob.buyTickets(4);
      await raffleCharlie.buyTickets(2); // Total = 10 = maxTickets
      
      // Should already be closed
      expect(await raffle.status()).to.equal(1n); // CLOSED
      
      await raffle.requestRandom();
      const reqId = await raffle.requestId();
      await rng.fulfill(reqId, 12345n);
      
      const bobBefore = await usdc.balanceOf(bob.address);
      const feeBefore = await usdc.balanceOf(feeRecipient.address);
      
      await raffle.finalize();
      
      const winner = await raffle.winner();
      expect(winner).to.not.equal("0x0000000000000000000000000000000000000000");
      
      // Verify payouts occurred
      const pot = ticketPrice * BigInt(maxTickets);
      const feeAmount = (pot * BigInt(feeBps)) / 10000n;
      const prizeAmount = pot - feeAmount;
      
      const feeAfter = await usdc.balanceOf(feeRecipient.address);
      expect(feeAfter - feeBefore).to.equal(feeAmount);
    });

    it("should not allow requestRandom with zero tickets", async function () {
      const { raffle, endTime, ethers } = await deployRaffleFixture();
      
      // Force close without any tickets (time travel)
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      
      let reverted = false;
      try {
        await raffle.requestRandom();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("NoTickets");
      }
      expect(reverted).to.equal(true);
    });

    it("should correctly handle multiple buyers with same winning index owner", async function () {
      const { raffle, rng, alice, bob, endTime, ethers, usdc, feeRecipient, ticketPrice, feeBps } = 
        await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      const raffleBob = raffle.connect(bob) as RaffleContract;
      
      // Alice: 0-2, Bob: 3-4
      await raffleAlice.buyTickets(3);
      await raffleBob.buyTickets(2);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      await raffle.requestRandom();
      
      const reqId = await raffle.requestId();
      // randomness=2 => winningIndex=2 => Alice (0..2)
      await rng.fulfill(reqId, 2n);
      
      const aliceBefore = await usdc.balanceOf(alice.address);
      await raffle.finalize();
      
      expect(await raffle.winner()).to.equal(alice.address);
      
      const pot = ticketPrice * 5n;
      const feeAmount = (pot * BigInt(feeBps)) / 10000n;
      const prizeAmount = pot - feeAmount;
      
      const aliceAfter = await usdc.balanceOf(alice.address);
      expect(aliceAfter - aliceBefore).to.equal(prizeAmount);
    });

    it("should handle canRefund view correctly", async function () {
      const { raffle, alice, bob, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(3);
      
      // Before close - cannot refund
      expect(await raffle.canRefund(alice.address)).to.equal(false);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      
      // After close but before delay - cannot refund
      expect(await raffle.canRefund(alice.address)).to.equal(false);
      
      const refundAt = await raffle.refundAvailableAt();
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(refundAt + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      // After delay - can refund
      expect(await raffle.canRefund(alice.address)).to.equal(true);
      
      // Bob has no tickets - cannot refund
      expect(await raffle.canRefund(bob.address)).to.equal(false);
      
      await raffleAlice.refund();
      
      // After refund - cannot refund again
      expect(await raffle.canRefund(alice.address)).to.equal(false);
    });

    it("should reject buying zero tickets", async function () {
      const { raffle, alice } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      
      let reverted = false;
      try {
        await raffleAlice.buyTickets(0);
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("InvalidTicketCount");
      }
      expect(reverted).to.equal(true);
    });

    it("should reject buying more than available tickets", async function () {
      const { raffle, alice, maxTickets } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      
      // Try to buy more than maxTickets in one purchase
      let reverted = false;
      try {
        await raffleAlice.buyTickets(maxTickets + 1);
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("SoldOut");
      }
      expect(reverted).to.equal(true);
    });

    it("should not allow close before endTime unless sold out", async function () {
      const { raffle, alice } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      // Try to close before endTime (not sold out)
      let reverted = false;
      try {
        await raffle.close();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("TooEarly");
      }
      expect(reverted).to.equal(true);
    });

    it("should correctly calculate refundAmount view", async function () {
      const { raffle, alice, bob, ticketPrice } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(5);
      
      // Alice should have 5 tickets worth of refund
      expect(await raffle.refundAmount(alice.address)).to.equal(ticketPrice * 5n);
      
      // Bob has no tickets
      expect(await raffle.refundAmount(bob.address)).to.equal(0n);
    });

    it("should not allow refund with zero tickets", async function () {
      const { raffle, alice, bob, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      const raffleBob = raffle.connect(bob) as RaffleContract;
      await raffleAlice.buyTickets(1);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      
      const refundAt = await raffle.refundAvailableAt();
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(refundAt + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      // Bob (no tickets) cannot refund
      let reverted = false;
      try {
        await raffleBob.refund();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("NoTickets");
      }
      expect(reverted).to.equal(true);
    });

    it("should not allow double refund", async function () {
      const { raffle, alice, endTime, ethers } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(2);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      
      const refundAt = await raffle.refundAvailableAt();
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(refundAt + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      // First refund succeeds
      await raffleAlice.refund();
      
      // Second refund fails
      let reverted = false;
      try {
        await raffleAlice.refund();
      } catch (err) {
        reverted = true;
        expect(String(err)).to.include("AlreadyRefunded");
      }
      expect(reverted).to.equal(true);
    });

    it("should correctly transfer refund amount to user", async function () {
      const { raffle, alice, usdc, endTime, ethers, ticketPrice } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(4);
      
      const aliceBalanceBefore = await usdc.balanceOf(alice.address);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      
      const refundAt = await raffle.refundAvailableAt();
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(refundAt + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffleAlice.refund();
      
      const aliceBalanceAfter = await usdc.balanceOf(alice.address);
      const refunded = aliceBalanceAfter - aliceBalanceBefore;
      
      expect(refunded).to.equal(ticketPrice * 4n);
    });

    it("should emit RefundClaimed with correct ticketCount", async function () {
      const { raffle, alice, ethers, raffleAddr, endTime, ticketPrice } = await deployRaffleFixture();
      
      const raffleAlice = raffle.connect(alice) as RaffleContract;
      await raffleAlice.buyTickets(3);
      
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(endTime + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      await raffle.close();
      
      const refundAt = await raffle.refundAvailableAt();
      await ethers.provider.send("evm_setNextBlockTimestamp", [Number(refundAt + 1n)]);
      await ethers.provider.send("evm_mine", []);
      
      const raffleContract = await ethers.getContractAt("Raffle", raffleAddr);
      const tx = await raffleAlice.refund();
      const receipt = await (tx as any).wait();
      
      // Find RefundClaimed event
      const iface = raffleContract.interface;
      const refundClaimedEvent = iface.getEvent("RefundClaimed")!;
      const refundClaimedTopic = refundClaimedEvent.topicHash;
      
      const refundLog = receipt.logs.find(
        (log: any) => log.topics[0] === refundClaimedTopic
      );
      expect(refundLog).to.not.be.undefined;
      
      // Decode the event data
      const decoded = iface.decodeEventLog("RefundClaimed", refundLog.data, refundLog.topics);
      expect(decoded.ticketCount).to.equal(3n);
      expect(decoded.amount).to.equal(ticketPrice * 3n);
    });
  });
});
