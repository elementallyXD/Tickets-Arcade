// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface IRandomnessProvider {
    /// @notice Request randomness for a given raffleId.
    /// @dev Provider should later call raffle.fulfillRandomness(requestId, randomness).
    function requestRandomness(uint256 raffleId) external returns (uint256 requestId);
}