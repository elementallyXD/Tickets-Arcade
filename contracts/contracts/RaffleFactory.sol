// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { Raffle } from "./Raffle.sol";

/// @title RaffleFactory
/// @notice Factory contract for deploying individual Raffle contracts
/// @dev Each raffle is a separate contract with its own lifecycle
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
    address public pendingRandomnessProvider;
    uint256 public pendingRandomnessProviderAt;

    uint16 public maxFeeBps; // global cap, e.g. 500 = 5%
    uint256 public nextRaffleId;

    address[] public raffles;

    uint256 public constant PROVIDER_UPDATE_DELAY = 1 days;

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
    event RandomnessProviderUpdateScheduled(address indexed newProvider, uint256 executeAfter);
    event MaxFeeBpsUpdated(uint16 oldMaxFeeBps, uint16 newMaxFeeBps);

    // -------------------------
    // Errors
    // -------------------------
    error InvalidAddress();
    error InvalidParams();
    error FeeTooHigh();
    error Unauthorized();
    error TooEarly();
    error NoPendingUpdate();

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
    
    /// @notice Returns total number of raffles created
    function rafflesCount() external view returns (uint256) {
        return raffles.length;
    }

    // -------------------------
    // Core logic
    // -------------------------
    
    /// @notice Create a new raffle
    /// @param endTime Unix timestamp when raffle closes
    /// @param ticketPrice Price per ticket in USDC smallest units
    /// @param maxTickets Maximum tickets available
    /// @param feeBps Fee in basis points (100 = 1%)
    /// @param feeRecipient Address to receive fees
    /// @return raffleAddr Address of deployed raffle contract
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

        address raffleCreator = msg.sender;
        uint256 raffleId = nextRaffleId;
        nextRaffleId = raffleId + 1;

        Raffle raffle = new Raffle(
            raffleId,
            usdc,
            randomnessProvider,
            raffleCreator,
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
            raffleCreator,
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
    
    /// @notice Schedule a new randomness provider (time-locked)
    /// @param newProvider Address of new VRF provider
    function setRandomnessProvider(address newProvider) external {
        if (msg.sender != admin) revert Unauthorized();
        if (newProvider == address(0)) revert InvalidAddress();

        pendingRandomnessProvider = newProvider;
        pendingRandomnessProviderAt = block.timestamp;

        emit RandomnessProviderUpdateScheduled(newProvider, block.timestamp + PROVIDER_UPDATE_DELAY);
    }

    /// @notice Apply pending randomness provider after delay
    function applyRandomnessProvider() external {
        if (msg.sender != admin) revert Unauthorized();
        address pendingProvider = pendingRandomnessProvider;
        if (pendingProvider == address(0)) revert NoPendingUpdate();
        if (block.timestamp < pendingRandomnessProviderAt + PROVIDER_UPDATE_DELAY) revert TooEarly();

        address previousProvider = randomnessProvider;
        randomnessProvider = pendingProvider;
        pendingRandomnessProvider = address(0);
        pendingRandomnessProviderAt = 0;

        emit RandomnessProviderUpdated(previousProvider, pendingProvider);
    }

    /// @notice Update maximum fee basis points allowed for new raffles
    /// @param newMaxFeeBps New maximum (cannot exceed 2000 = 20%)
    function setMaxFeeBps(uint16 newMaxFeeBps) external {
        if (msg.sender != admin) revert Unauthorized();
        if (newMaxFeeBps > 2000) revert InvalidParams();

        uint16 old = maxFeeBps;
        maxFeeBps = newMaxFeeBps;

        emit MaxFeeBpsUpdated(old, newMaxFeeBps);
    }
}
