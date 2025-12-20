// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { Raffle } from "./Raffle.sol";

contract RaffleFactory {
    // -------------------------
    // Admin
    // -------------------------
    address public immutable admin;

    // -------------------------
    // Configuration
    // -------------------------
    address public immutable usdc;
    address public randomnessProvider;

    uint16 public maxFeeBps; // global cap, e.g. 500 = 5%
    uint256 public nextRaffleId;

    address[] public raffles;

    // -------------------------
    // Events
    // -------------------------
    event RaffleCreated(
        uint256 indexed raffleId,
        address indexed raffle,
        address indexed creator,
        uint256 endTime,
        uint256 ticketPrice,
        uint32 maxTickets,
        uint16 feeBps,
        address feeRecipient
    );

    event RandomnessProviderUpdated(address indexed oldProvider, address indexed newProvider);
    event MaxFeeBpsUpdated(uint16 oldMaxFeeBps, uint16 newMaxFeeBps);

    // -------------------------
    // Errors
    // -------------------------
    error InvalidAddress();
    error InvalidParams();
    error FeeTooHigh();
    error Unauthorized();

    // -------------------------
    // Constructor
    // -------------------------
    constructor(
        address _usdc,
        address _randomnessProvider,
        uint16 _maxFeeBps
    ) {
        if (_usdc == address(0) || _randomnessProvider == address(0)) revert InvalidAddress();
        if (_maxFeeBps > 2000) revert InvalidParams(); // hard cap 20%

        admin = msg.sender;

        usdc = _usdc;
        randomnessProvider = _randomnessProvider;
        maxFeeBps = _maxFeeBps;

        nextRaffleId = 1;
    }

    // -------------------------
    // Views
    // -------------------------
    function rafflesCount() external view returns (uint256) {
        return raffles.length;
    }

    // -------------------------
    // Core logic
    // -------------------------
    function createRaffle(
        uint256 endTime,
        uint256 ticketPrice,
        uint32 maxTickets,
        uint16 feeBps,
        address feeRecipient
    ) external returns (address raffleAddr) {
        if (feeRecipient == address(0)) revert InvalidAddress();
        if (ticketPrice == 0 || maxTickets == 0) revert InvalidParams();
        if (endTime <= block.timestamp) revert InvalidParams();
        if (feeBps > maxFeeBps) revert FeeTooHigh();

        address creator = msg.sender;
        uint256 raffleId = nextRaffleId;
        nextRaffleId = raffleId + 1;

        Raffle raffle = new Raffle(
            raffleId,
            usdc,
            randomnessProvider,
            creator,
            endTime,
            ticketPrice,
            maxTickets,
            feeBps,
            feeRecipient
        );

        raffleAddr = address(raffle);
        raffles.push(raffleAddr);

        emit RaffleCreated(
            raffleId,
            raffleAddr,
            creator,
            endTime,
            ticketPrice,
            maxTickets,
            feeBps,
            feeRecipient
        );
    }

    // -------------------------
    // Admin controls
    // -------------------------
    function setRandomnessProvider(address newProvider) external {
        if (msg.sender != admin) revert Unauthorized();
        if (newProvider == address(0)) revert InvalidAddress();

        address old = randomnessProvider;
        randomnessProvider = newProvider;

        emit RandomnessProviderUpdated(old, newProvider);
    }

    function setMaxFeeBps(uint16 newMaxFeeBps) external {
        if (msg.sender != admin) revert Unauthorized();
        if (newMaxFeeBps > 2000) revert InvalidParams();

        uint16 old = maxFeeBps;
        maxFeeBps = newMaxFeeBps;

        emit MaxFeeBpsUpdated(old, newMaxFeeBps);
    }
}
