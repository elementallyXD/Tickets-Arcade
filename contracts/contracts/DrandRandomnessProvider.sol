// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { IRandomnessProvider } from "./IRandomnessProvider.sol";

// Interface for Raffle contracts to receive fulfilled randomness
interface IRaffleFulfill {
    function fulfillRandomness(uint256 requestId, uint256 randomness) external;
}

/// @notice Adapter for off-chain randomness sources (e.g., drand).
/// The oracle submits randomness and optional proof; on-chain verification
/// can be added later without changing the raffle flow.
contract DrandRandomnessProvider is IRandomnessProvider {
    address public immutable oracle;
    uint256 public nextRequestId = 1;

    // requestId => raffle address
    mapping(uint256 => address) public requestToRaffle;
    mapping(uint256 => bool) public fulfilled;

    event RandomnessRequested(uint256 indexed requestId, uint256 indexed raffleId, address indexed raffle);
    event RandomnessDelivered(uint256 indexed requestId, uint256 randomness, bytes proof, address indexed raffle);

    error Unauthorized();
    error InvalidRequest();
    error AlreadyFulfilled();

    constructor(address _oracle) {
        if (_oracle == address(0)) revert Unauthorized();
        oracle = _oracle;
    }

    function requestRandomness(uint256 raffleId) external override returns (uint256 requestId) {
        requestId = nextRequestId++;
        requestToRaffle[requestId] = msg.sender;

        emit RandomnessRequested(requestId, raffleId, msg.sender);
    }

    /// @notice Oracle-delivered randomness with optional proof data.
    function deliverRandomness(uint256 requestId, uint256 randomness, bytes calldata proof) external {
        if (msg.sender != oracle) revert Unauthorized();

        address raffle = requestToRaffle[requestId];
        if (raffle == address(0)) revert InvalidRequest();
        if (fulfilled[requestId]) revert AlreadyFulfilled();
        if (randomness == 0) revert InvalidRequest();

        fulfilled[requestId] = true;

        IRaffleFulfill(raffle).fulfillRandomness(requestId, randomness);

        emit RandomnessDelivered(requestId, randomness, proof, raffle);
    }
}
