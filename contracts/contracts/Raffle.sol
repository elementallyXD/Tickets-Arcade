// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/**
 * @title Raffle
 * @author Ticket Arcade Team
 * @notice Individual raffle contract deployed by RaffleFactory
 * @dev Lifecycle: ACTIVE -> CLOSED -> RANDOM_REQUESTED -> RANDOM_FULFILLED -> FINALIZED
 *      Refund path available from CLOSED or RANDOM_REQUESTED after REFUND_DELAY
 */

import { IERC20Minimal } from "./IERC20Minimal.sol";
import { IRandomnessProvider } from "./IRandomnessProvider.sol";

contract Raffle {
    // ═══════════════════════════════════════════════════════════════════════════
    // TYPES
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// @notice Raffle lifecycle states
    /// @dev State transitions are strictly enforced:
    ///      ACTIVE -> CLOSED (via close() or sold out)
    ///      CLOSED -> RANDOM_REQUESTED (via requestRandom())
    ///      RANDOM_REQUESTED -> RANDOM_FULFILLED (via VRF callback)
    ///      RANDOM_FULFILLED -> FINALIZED (via finalize())
    enum Status {
        ACTIVE,            // Accepting ticket purchases
        CLOSED,            // No more purchases, awaiting randomness request
        RANDOM_REQUESTED,  // VRF request sent, awaiting callback
        RANDOM_FULFILLED,  // Randomness received, ready to finalize
        FINALIZED          // Winner paid, raffle complete
    }

    /// @notice Represents a contiguous range of tickets owned by a buyer
    /// @dev Ranges are stored in order of purchase for O(log n) winner lookup
    struct TicketRange {
        address buyer;  // Owner of tickets in this range
        uint32 start;   // First ticket index (inclusive)
        uint32 end;     // Last ticket index (inclusive)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // IMMUTABLE CONFIGURATION
    // ═══════════════════════════════════════════════════════════════════════════
    
    // --- Addresses ---
    IERC20Minimal public immutable usdc;           // Payment token (6 decimals)
    address public immutable factory;               // Factory that deployed this raffle
    address public immutable creator;               // Raffle creator (has admin rights)
    address public immutable randomnessProvider;    // VRF provider for winner selection
    address public immutable feeRecipient;          // Receives protocol fees
    
    // --- Raffle Parameters ---
    uint256 public immutable raffleId;      // Unique identifier from factory
    uint256 public immutable endTime;       // Unix timestamp when purchases end
    uint256 public immutable ticketPrice;   // Price per ticket in USDC smallest units
    uint32 public immutable maxTickets;     // Maximum tickets available
    uint16 public immutable feeBps;         // Fee in basis points (100 = 1%, max 1000 = 10%)
    
    // --- Constants ---
    /// @notice Time after endTime before refunds become available
    uint256 public constant REFUND_DELAY = 1 days;

    // ═══════════════════════════════════════════════════════════════════════════
    // MUTABLE STATE
    // ═══════════════════════════════════════════════════════════════════════════
    
    // --- Lifecycle ---
    Status public status;                   // Current raffle state
    address public keeper;                  // Secondary operator (can be changed by creator)
    
    // --- Ticket Tracking ---
    uint32 public totalTickets;             // Number of tickets sold
    uint256 public pot;                     // Total USDC collected
    TicketRange[] public ranges;            // Ordered list of ticket ownership ranges
    mapping(address => uint32) public ticketsByBuyer;  // Tickets per address (for refunds)
    
    // --- Randomness ---
    uint256 public requestId;               // VRF request identifier
    uint256 public randomness;              // Raw random value from VRF
    uint256 public winningIndex;            // Computed as randomness % totalTickets
    address public winner;                  // Address of winner (set on finalize)
    
    // --- Refund State ---
    mapping(address => bool) public refunded;  // Track who has claimed refund
    bool public refundsEnabled;             // Set true on first refund, blocks finalization
    
    // --- Reentrancy Guard ---
    bool private _locked;

    // ═══════════════════════════════════════════════════════════════════════════
    // EVENTS
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// @notice Emitted when tickets are purchased
    event TicketsBought(
        uint256 indexed raffleId,
        address indexed buyer,
        uint32 startIndex,
        uint32 endIndex,
        uint32 count,
        uint256 amountPaid
    );

    /// @notice Emitted when raffle moves from ACTIVE to CLOSED
    event RaffleClosed(uint256 indexed raffleId, uint256 totalTickets, uint256 pot);
    
    /// @notice Emitted when VRF randomness is requested
    event RandomnessRequested(uint256 indexed raffleId, uint256 requestId);
    
    /// @notice Emitted when VRF delivers randomness
    event RandomnessFulfilled(uint256 indexed raffleId, uint256 requestId, uint256 randomness);
    
    /// @notice Emitted when winner is determined and paid
    event WinnerSelected(
        uint256 indexed raffleId,
        address indexed winner,
        uint256 winningIndex,
        uint256 prizeAmount,
        uint256 feeAmount
    );

    /// @notice Emitted after all payouts complete
    event PayoutsCompleted(
        uint256 indexed raffleId,
        address indexed winner,
        address indexed feeRecipient,
        uint256 prizeAmount,
        uint256 feeAmount
    );

    /// @notice Emitted when keeper address is updated
    event KeeperUpdated(address indexed oldKeeper, address indexed newKeeper);
    
    /// @notice Emitted when a buyer claims their refund
    event RefundClaimed(uint256 indexed raffleId, address indexed buyer, uint32 ticketCount, uint256 amount);
    
    /// @notice Emitted once when the first refund is claimed (blocks finalization)
    event RefundsStarted(uint256 indexed raffleId, uint256 timestamp);

    // ═══════════════════════════════════════════════════════════════════════════
    // ERRORS
    // ═══════════════════════════════════════════════════════════════════════════
    
    // Status errors
    error NotActive();              // Raffle not in ACTIVE status
    error NotClosed();              // Raffle not in CLOSED status  
    error NotRandomRequested();     // Raffle not in RANDOM_REQUESTED status
    error NotRandomFulfilled();     // Raffle not in RANDOM_FULFILLED status
    
    // Timing errors
    error TooLate();                // Action attempted after deadline
    error TooEarly();               // Action attempted before allowed
    
    // Purchase errors
    error InvalidTicketCount();     // Zero tickets requested
    error SoldOut();                // No tickets remaining
    
    // Authorization errors
    error Unauthorized();           // Caller lacks permission
    
    // Operation errors
    error InvalidRequest();         // Generic invalid operation
    error NoTickets();              // No tickets to operate on
    error WinnerNotFound();         // Binary search failed (should never happen)
    
    // Refund errors
    error RefundsAlreadyEnabled();  // Cannot finalize after refunds started
    error RefundsNotAvailable();    // Cannot refund in current state
    error AlreadyRefunded();        // Buyer already claimed refund
    
    // Balance errors
    error InsufficientPot();        // Refund exceeds pot
    error InsufficientBalance();    // Contract USDC balance too low
    
    // Reentrancy
    error ReentrancyGuard();        // Reentrant call detected

    // ═══════════════════════════════════════════════════════════════════════════
    // MODIFIERS
    // ═══════════════════════════════════════════════════════════════════════════

    /// @dev Restricts to raffle creator only
    modifier onlyCreator() {
        if (msg.sender != creator) revert Unauthorized();
        _;
    }

    /// @dev Restricts to creator or keeper
    modifier onlyOperator() {
        if (msg.sender != creator && msg.sender != keeper) revert Unauthorized();
        _;
    }

    /// @dev Restricts to factory, creator, or keeper
    modifier onlyOperatorOrFactory() {
        if (msg.sender != factory && msg.sender != creator && msg.sender != keeper) revert Unauthorized();
        _;
    }

    /// @dev Prevents reentrancy attacks
    modifier nonReentrant() {
        if (_locked) revert ReentrancyGuard();
        _locked = true;
        _;
        _locked = false;
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // CONSTRUCTOR
    // ═══════════════════════════════════════════════════════════════════════════

    /// @notice Deploy a new raffle (called by RaffleFactory)
    /// @param _raffleId Unique identifier from factory
    /// @param _usdc USDC token address
    /// @param _randomnessProvider VRF provider address
    /// @param _creator Raffle creator/admin address
    /// @param _endTime Unix timestamp when purchases end
    /// @param _ticketPrice Price per ticket in USDC smallest units
    /// @param _maxTickets Maximum tickets available
    /// @param _feeBps Fee in basis points (max 1000 = 10%)
    /// @param _feeRecipient Address to receive fees
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

        // Validate addresses (all required)
        if (_usdc == address(0) || _randomnessProvider == address(0) || _feeRecipient == address(0) || _creator == address(0)) {
            revert Unauthorized();
        }
        
        // Validate raffle parameters
        if (_ticketPrice == 0 || _maxTickets == 0) {
            revert InvalidTicketCount();
        }
        
        // endTime should be in the future
        if (_endTime <= block.timestamp) {
            revert TooEarly();
        }
        
        // feeBps capped at 10% (factory may enforce lower)
        if (_feeBps > 1000) {
            revert InvalidRequest();
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // VIEW FUNCTIONS
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// @notice Returns the number of ticket purchase ranges
    function rangesCount() external view returns (uint256) {
        return ranges.length;
    }

    /// @notice Returns the timestamp when refunds become available
    function refundAvailableAt() public view returns (uint256) {
        return endTime + REFUND_DELAY;
    }

    /// @notice Calculate refund amount for a buyer
    /// @param buyer Address to check
    /// @return Refund amount in USDC units, or 0 if already refunded
    function refundAmount(address buyer) public view returns (uint256) {
        if (refunded[buyer]) {
            return 0;
        }
        return ticketPrice * uint256(ticketsByBuyer[buyer]);
    }

    /// @notice Check if a buyer can currently claim a refund
    /// @param buyer Address to check
    /// @return True if refund is available
    function canRefund(address buyer) external view returns (bool) {
        if (refunded[buyer]) return false;
        if (ticketsByBuyer[buyer] == 0) return false;
        if (!(status == Status.CLOSED || status == Status.RANDOM_REQUESTED)) return false;
        if (block.timestamp < refundAvailableAt()) return false;
        return true;
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // USER ACTIONS
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// @notice Purchase tickets for the raffle
    /// @param count Number of tickets to purchase (must be > 0)
    /// @dev Transfers USDC from caller. Auto-closes raffle if sold out.
    function buyTickets(uint32 count) external nonReentrant {
        // Validate state and timing
        if (status != Status.ACTIVE) revert NotActive();
        if (block.timestamp >= endTime) revert TooLate();
        
        // Validate count
        if (count == 0) revert InvalidTicketCount();
        
        // Check ticket availability
        if (totalTickets >= maxTickets) revert SoldOut();
        if (uint256(totalTickets) + uint256(count) > uint256(maxTickets)) revert SoldOut();

        // Calculate and collect payment
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

    /// @notice Close the raffle. Only operator or factory once conditions are met.
    /// @dev Conditions: endTime passed OR all tickets sold
    function close() external onlyOperatorOrFactory {
        _close();
    }

    /// @dev Internal close logic - validates conditions and updates state
    function _close() internal {
        if (status != Status.ACTIVE) revert NotActive();
        
        // Require: time expired OR sold out
        bool timeExpired = block.timestamp >= endTime;
        bool soldOut = totalTickets == maxTickets;
        if (!timeExpired && !soldOut) revert TooEarly();

        status = Status.CLOSED;
        emit RaffleClosed(raffleId, totalTickets, pot);
    }

    /// @notice Update the keeper address. Only creator can call.
    /// @param newKeeper New keeper address (can be address(0) to disable)
    function setKeeper(address newKeeper) external onlyCreator {
        address oldKeeper = keeper;
        keeper = newKeeper;
        emit KeeperUpdated(oldKeeper, newKeeper);
    }

    /// @notice Claim refund if randomness was never fulfilled within delay period.
    /// @dev Once any refund is claimed, refundsEnabled=true permanently blocks finalization.
    ///      Follows CEI pattern to prevent reentrancy.
    function refund() external nonReentrant {
        // Validate state - only CLOSED or RANDOM_REQUESTED allow refunds
        if (!(status == Status.CLOSED || status == Status.RANDOM_REQUESTED)) {
            revert RefundsNotAvailable();
        }
        
        // Validate timing - must wait REFUND_DELAY after endTime
        if (block.timestamp < refundAvailableAt()) revert TooEarly();
        
        // Validate caller hasn't already refunded
        if (refunded[msg.sender]) revert AlreadyRefunded();

        // Get caller's tickets
        uint32 ticketCount = ticketsByBuyer[msg.sender];
        if (ticketCount == 0) revert NoTickets();

        // Calculate refund amount
        uint256 refundValue = ticketPrice * uint256(ticketCount);
        
        // Validate sufficient funds
        if (refundValue > pot) revert InsufficientPot();
        if (usdc.balanceOf(address(this)) < refundValue) revert InsufficientBalance();

        // === EFFECTS (before external calls) ===
        refunded[msg.sender] = true;
        pot -= refundValue;
        
        bool isFirstRefund = !refundsEnabled;
        if (isFirstRefund) {
            refundsEnabled = true;  // Permanently blocks finalization
        }

        // === INTERACTIONS (external calls) ===
        bool transferSuccess = usdc.transfer(msg.sender, refundValue);
        if (!transferSuccess) revert InvalidRequest();

        // Emit events
        if (isFirstRefund) {
            emit RefundsStarted(raffleId, block.timestamp);
        }
        emit RefundClaimed(raffleId, msg.sender, ticketCount, refundValue);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // RANDOMNESS FLOW
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// @notice Request randomness from VRF provider. Only operator can call.
    /// @dev Must be in CLOSED status with tickets sold. Transitions to RANDOM_REQUESTED.
    function requestRandom() external onlyOperator nonReentrant {
        // Block if refunds have started
        if (refundsEnabled) revert RefundsAlreadyEnabled();
        
        // Validate state
        if (status != Status.CLOSED) revert NotClosed();
        if (totalTickets == 0) revert NoTickets();

        // Update state before external call
        status = Status.RANDOM_REQUESTED;

        // Request randomness from VRF provider
        uint256 newRequestId = IRandomnessProvider(randomnessProvider).requestRandomness(raffleId);
        requestId = newRequestId;

        emit RandomnessRequested(raffleId, newRequestId);
    }

    /// @notice Callback from VRF provider with randomness.
    /// @dev Only randomnessProvider can call. Must match requestId.
    /// @param _requestId The request ID from requestRandom()
    /// @param _randomness The random value (must be non-zero)
    function fulfillRandomness(uint256 _requestId, uint256 _randomness) external {
        // Block if refunds have started
        if (refundsEnabled) revert RefundsAlreadyEnabled();
        
        // Validate caller and state
        if (msg.sender != randomnessProvider) revert Unauthorized();
        if (status != Status.RANDOM_REQUESTED) revert NotRandomRequested();
        
        // Validate request
        if (_requestId != requestId) revert InvalidRequest();
        if (_randomness == 0) revert InvalidRequest();

        // Store randomness and compute winner
        randomness = _randomness;
        winningIndex = _randomness % uint256(totalTickets);

        status = Status.RANDOM_FULFILLED;

        emit RandomnessFulfilled(raffleId, _requestId, _randomness);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // FINALIZATION
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// @notice Finalize the raffle, pay winner and fees. Only operator can call.
    /// @dev Follows CEI pattern: state updates before external transfers.
    function finalize() external onlyOperator nonReentrant {
        // Block if refunds have started
        if (refundsEnabled) revert RefundsAlreadyEnabled();
        
        // Validate state (status check sufficient - winner only set in finalize)
        if (status != Status.RANDOM_FULFILLED) revert NotRandomFulfilled();

        // Find winner using binary search
        address winnerAddress = _findWinner(uint32(winningIndex));
        if (winnerAddress == address(0)) revert WinnerNotFound();

        // Validate contract has sufficient balance
        uint256 currentPot = pot;
        if (usdc.balanceOf(address(this)) < currentPot) revert InsufficientBalance();

        // Calculate payouts
        uint256 feeAmount = (currentPot * uint256(feeBps)) / 10_000;
        uint256 prizeAmount = currentPot - feeAmount;

        // === EFFECTS (before external calls) ===
        winner = winnerAddress;
        status = Status.FINALIZED;
        pot = 0;

        // === INTERACTIONS (external calls) ===
        if (prizeAmount > 0) {
            bool prizeSuccess = usdc.transfer(winnerAddress, prizeAmount);
            if (!prizeSuccess) revert InvalidRequest();
        }

        if (feeAmount > 0) {
            bool feeSuccess = usdc.transfer(feeRecipient, feeAmount);
            if (!feeSuccess) revert InvalidRequest();
        }

        // Emit events in logical order
        emit WinnerSelected(raffleId, winnerAddress, winningIndex, prizeAmount, feeAmount);
        emit PayoutsCompleted(raffleId, winnerAddress, feeRecipient, prizeAmount, feeAmount);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // INTERNAL HELPERS
    // ═══════════════════════════════════════════════════════════════════════════

    /// @notice Find the buyer who owns a specific ticket index
    /// @dev Uses binary search for O(log n) lookup over sorted ranges
    /// @param ticketIndex The ticket index to look up
    /// @return The buyer address, or address(0) if not found
    function _findWinner(uint32 ticketIndex) internal view returns (address) {
        uint256 low = 0;
        uint256 high = ranges.length;
        
        while (low < high) {
            uint256 mid = (low + high) / 2;
            TicketRange memory range = ranges[mid];
            
            if (ticketIndex < range.start) {
                // Ticket is before this range
                high = mid;
            } else if (ticketIndex > range.end) {
                // Ticket is after this range
                low = mid + 1;
            } else {
                // Ticket is within this range
                return range.buyer;
            }
        }
        
        // Should never happen if ranges are properly maintained
        return address(0);
    }
}
