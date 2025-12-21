// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { IERC20Minimal } from "./IERC20Minimal.sol";
import { IRandomnessProvider } from "./IRandomnessProvider.sol";

contract Raffle {
    // -------------------------
    // Types
    // -------------------------
    enum Status {
        ACTIVE,
        CLOSED,
        RANDOM_REQUESTED,
        RANDOM_FULFILLED,
        FINALIZED
    }

    struct TicketRange {
        address buyer;
        uint32 start;
        uint32 end;
    }

    // -------------------------
    // Immutable configuration
    // -------------------------
    IERC20Minimal public immutable usdc;
    address public immutable factory;
    address public immutable creator;
    address public immutable randomnessProvider;

    uint256 public immutable raffleId;
    uint256 public immutable endTime;
    uint256 public immutable ticketPrice;  // in USDC smallest units
    uint32  public immutable maxTickets;   // hard cap
    uint16  public immutable feeBps;       // 100 = 1%
    address public immutable feeRecipient;

    uint256 public constant REFUND_DELAY = 1 days;

    // -------------------------
    // Mutable state
    // -------------------------
    Status public status;

    address public keeper;

    uint32 public totalTickets;
    uint256 public pot;

    uint256 public requestId;
    uint256 public randomness;
    uint256 public winningIndex;
    address public winner;

    TicketRange[] public ranges;

    mapping(address => uint32) public ticketsByBuyer;
    mapping(address => bool) public refunded;
    bool public refundsEnabled;

    // -------------------------
    // Events
    // -------------------------
    event TicketsBought(
        uint256 indexed raffleId,
        address indexed buyer,
        uint32 startIndex,
        uint32 endIndex,
        uint32 count,
        uint256 amountPaid
    );

    event RaffleClosed(uint256 indexed raffleId, uint256 totalTickets, uint256 pot);
    event RandomnessRequested(uint256 indexed raffleId, uint256 requestId);
    event RandomnessFulfilled(uint256 indexed raffleId, uint256 requestId, uint256 randomness);
    event WinnerSelected(
        uint256 indexed raffleId,
        address indexed winner,
        uint256 winningIndex,
        uint256 prizeAmount,
        uint256 feeAmount
    );

    event PayoutsCompleted(
        uint256 indexed raffleId,
        address indexed winner,
        address indexed feeRecipient,
        uint256 prizeAmount,
        uint256 feeAmount
    );

    event KeeperUpdated(address oldKeeper, address newKeeper);
    event RefundClaimed(uint256 raffleId, address buyer, uint256 amount);

    // -------------------------
    // Errors (cheaper than revert strings)
    // -------------------------
    error NotActive();
    error NotClosed();
    error NotRandomRequested();
    error NotRandomFulfilled();
    error TooLate();
    error TooEarly();
    error InvalidCount();
    error SoldOut();
    error NoTickets();
    error Unauthorized();
    error InvalidRequest();
    error AlreadyFinalized();
    error RefundsEnabled();
    error RefundsNotAvailable();
    error AlreadyRefunded();
    error Reentrancy();
    error InsufficientPot();
    error InsufficientBalance();

    bool private locked;

    modifier onlyCreator() {
        if (msg.sender != creator) revert Unauthorized();
        _;
    }

    modifier onlyOperator() {
        if (msg.sender != creator && msg.sender != keeper) revert Unauthorized();
        _;
    }

    modifier onlyOperatorOrFactory() {
        if (msg.sender != factory && msg.sender != creator && msg.sender != keeper) revert Unauthorized();
        _;
    }

    modifier nonReentrant() {
        if (locked) revert Reentrancy();
        locked = true;
        _;
        locked = false;
    }

    constructor(
        uint256 _raffleId,
        address _usdc,
        address _randomnessProvider,
        address _creator,
        uint256 _endTime,
        uint256 _ticketPrice,
        uint32 _maxTickets,
        uint16 _feeBps,
        address _feeRecipient
    ) {
        factory = msg.sender; // deployed by factory
        creator = _creator;
        raffleId = _raffleId;

        usdc = IERC20Minimal(_usdc);
        randomnessProvider = _randomnessProvider;

        endTime = _endTime;
        ticketPrice = _ticketPrice;
        maxTickets = _maxTickets;

        feeBps = _feeBps;
        feeRecipient = _feeRecipient;

        status = Status.ACTIVE;
        keeper = _creator;

        // basic sanity checks
        if (_usdc == address(0) || _randomnessProvider == address(0) || _feeRecipient == address(0) || _creator == address(0)) {
            revert Unauthorized();
        }
        if (_ticketPrice == 0 || _maxTickets == 0) {
            revert InvalidCount();
        }
        // endTime should be in the future (factory can enforce too)
        if (_endTime <= block.timestamp) {
            revert TooEarly();
        }
        // feeBps should be reasonable (factory should enforce as well)
        if (_feeBps > 1000) { // max 10%
            revert InvalidRequest();
        }
    }

    // -------------------------
    // Views
    // -------------------------
    function rangesCount() external view returns (uint256) {
        return ranges.length;
    }

    function refundAvailableAt() public view returns (uint256) {
        return endTime + REFUND_DELAY;
    }

    function refundAmount(address buyer) public view returns (uint256) {
        if (refunded[buyer]) {
            return 0;
        }
        return ticketPrice * uint256(ticketsByBuyer[buyer]);
    }

    function canRefund(address buyer) external view returns (bool) {
        if (refunded[buyer]) return false;
        if (ticketsByBuyer[buyer] == 0) return false;
        if (!(status == Status.CLOSED || status == Status.RANDOM_REQUESTED)) return false;
        if (block.timestamp < refundAvailableAt()) return false;
        return true;
    }

    // -------------------------
    // User actions
    // -------------------------
    function buyTickets(uint32 count) external nonReentrant {
        if (status != Status.ACTIVE) revert NotActive();
        if (block.timestamp >= endTime) revert TooLate();
        if (count == 0) revert InvalidCount();
        if (totalTickets >= maxTickets) revert SoldOut();
        if (uint256(totalTickets) + uint256(count) > uint256(maxTickets)) revert SoldOut();

        uint256 cost = ticketPrice * uint256(count);

        bool ok = usdc.transferFrom(msg.sender, address(this), cost);
        if (!ok) revert InvalidRequest();

        uint32 startIndex = totalTickets;
        uint32 endIndex = startIndex + count - 1;

        ranges.push(TicketRange({ buyer: msg.sender, start: startIndex, end: endIndex }));
        ticketsByBuyer[msg.sender] += count;

        totalTickets += count;
        pot += cost;

        emit TicketsBought(raffleId, msg.sender, startIndex, endIndex, count, cost);

        // Auto-close if sold out
        if (totalTickets == maxTickets) {
            _close();
        }
    }

    function close() external onlyOperatorOrFactory {
        _close();
    }

    function _close() internal {
        if (status != Status.ACTIVE) revert NotActive();
        // Allow close if time passed OR sold out
        if (!(block.timestamp >= endTime || totalTickets == maxTickets)) revert TooEarly();

        status = Status.CLOSED;
        emit RaffleClosed(raffleId, totalTickets, pot);
    }

    function setKeeper(address newKeeper) external onlyCreator {
        address oldKeeper = keeper;
        keeper = newKeeper;

        emit KeeperUpdated(oldKeeper, newKeeper);
    }

    function refund() external nonReentrant {
        if (!(status == Status.CLOSED || status == Status.RANDOM_REQUESTED)) revert RefundsNotAvailable();
        if (block.timestamp < refundAvailableAt()) revert TooEarly();
        if (refunded[msg.sender]) revert AlreadyRefunded();

        uint32 tickets = ticketsByBuyer[msg.sender];
        if (tickets == 0) revert NoTickets();

        uint256 refundValue = ticketPrice * uint256(tickets);
        if (refundValue > pot) revert InsufficientPot();
        if (usdc.balanceOf(address(this)) < refundValue) revert InsufficientBalance();

        refunded[msg.sender] = true;
        if (!refundsEnabled) {
            refundsEnabled = true;
        }

        pot -= refundValue;

        bool ok = usdc.transfer(msg.sender, refundValue);
        if (!ok) revert InvalidRequest();

        emit RefundClaimed(raffleId, msg.sender, refundValue);
    }

    // -------------------------
    // Randomness flow
    // -------------------------
    function requestRandom() external onlyOperator nonReentrant {
        if (refundsEnabled) revert RefundsEnabled();
        if (status != Status.CLOSED) revert NotClosed();
        if (totalTickets == 0) revert NoTickets();

        status = Status.RANDOM_REQUESTED;

        uint256 requestIdValue = IRandomnessProvider(randomnessProvider).requestRandomness(raffleId);
        requestId = requestIdValue;

        emit RandomnessRequested(raffleId, requestIdValue);
    }

    /// @notice Called by the randomness provider (VRF) when randomness is ready.
    function fulfillRandomness(uint256 _requestId, uint256 _randomness) external {
        if (refundsEnabled) revert RefundsEnabled();
        if (msg.sender != randomnessProvider) revert Unauthorized();
        if (status != Status.RANDOM_REQUESTED) revert NotRandomRequested();
        if (_requestId != requestId) revert InvalidRequest();
        if (_randomness == 0) revert InvalidRequest();

        randomness = _randomness;
        winningIndex = _randomness % uint256(totalTickets);

        status = Status.RANDOM_FULFILLED;

        emit RandomnessFulfilled(raffleId, _requestId, _randomness);
    }

    // -------------------------
    // Finalization / payout
    // -------------------------
    function finalize() external onlyOperator nonReentrant {
        if (refundsEnabled) revert RefundsEnabled();
        if (status != Status.RANDOM_FULFILLED) revert NotRandomFulfilled();
        if (winner != address(0)) revert AlreadyFinalized();

        address winnerAddress = _findWinner(uint32(winningIndex));
        winner = winnerAddress;

        uint256 feeAmount = (pot * uint256(feeBps)) / 10000;
        uint256 prizeAmount = pot - feeAmount;
        if (usdc.balanceOf(address(this)) < pot) revert InsufficientBalance();

        // effects done, now interactions
        if (prizeAmount > 0) {
            bool ok1 = usdc.transfer(winnerAddress, prizeAmount);
            if (!ok1) revert InvalidRequest();
        }

        if (feeAmount > 0) {
            bool ok2 = usdc.transfer(feeRecipient, feeAmount);
            if (!ok2) revert InvalidRequest();
        }

        status = Status.FINALIZED;

        emit PayoutsCompleted(raffleId, winnerAddress, feeRecipient, prizeAmount, feeAmount);
        emit WinnerSelected(raffleId, winnerAddress, winningIndex, prizeAmount, feeAmount);
    }

    // -------------------------
    // Internal helper
    // -------------------------
    function _findWinner(uint32 idx) internal view returns (address) {
        // Binary search over contiguous, ordered ranges.
        uint256 low = 0;
        uint256 high = ranges.length;
        while (low < high) {
            uint256 mid = (low + high) / 2;
            TicketRange memory range = ranges[mid];
            if (idx < range.start) {
                high = mid;
            } else if (idx > range.end) {
                low = mid + 1;
            } else {
                return range.buyer;
            }
        }
        // Should be impossible if ranges are correct
        return address(0);
    }
}
