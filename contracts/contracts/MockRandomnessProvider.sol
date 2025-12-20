// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { IRandomnessProvider } from "./IRandomnessProvider.sol";

interface IRaffleFulfill {
    function fulfillRandomness(uint256 requestId, uint256 randomness) external;
}

/// @notice Test-only randomness provider.
/// - requestRandomness() returns an incrementing requestId and remembers which raffle requested it.
/// - fulfill(requestId, randomness) calls the raffle back.
contract MockRandomnessProvider is IRandomnessProvider {
    uint256 public nextRequestId = 1;

    // requestId => raffle address
    mapping(uint256 => address) public requestToRaffle;

    event RandomnessRequested(uint256 indexed requestId, uint256 indexed raffleId, address indexed raffle);
    event RandomnessDelivered(uint256 indexed requestId, uint256 randomness, address indexed raffle);

    function requestRandomness(uint256 raffleId) external override returns (uint256 requestId) {
        requestId = nextRequestId++;
        requestToRaffle[requestId] = msg.sender; // msg.sender is the Raffle contract

        emit RandomnessRequested(requestId, raffleId, msg.sender);
    }

    /// @notice Anyone can call in tests. In production VRF, only oracle would call.
    function fulfill(uint256 requestId, uint256 randomness) external {
        address raffle = requestToRaffle[requestId];
        require(raffle != address(0), "unknown requestId");
        require(randomness != 0, "randomness=0");

        IRaffleFulfill(raffle).fulfillRandomness(requestId, randomness);

        emit RandomnessDelivered(requestId, randomness, raffle);
    }
}
