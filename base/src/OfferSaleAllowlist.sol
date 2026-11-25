// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";

/**
 * @title OfferSaleAllowlist
 * @notice Global registry that caps how much BRLA each buyer can spend on Offer Tokens.
 * @custom:oz-upgrades-unsafe-allow constructor
 */
contract OfferSaleAllowlist is Initializable, OwnableUpgradeable, UUPSUpgradeable {
    struct Allocation {
        uint256 maxAllocation;
        uint256 consumed;
        bool enabled;
    }

    mapping(address => Allocation) private allocations;
    mapping(address => bool) public controllers;

    modifier onlyController() {
        require(controllers[msg.sender], "Unauthorized controller");
        _;
    }

    event ControllerUpdated(address indexed controller, bool allowed);
    event AllocationUpdated(address indexed buyer, uint256 maxAllocation, bool enabled);
    event AllocationConsumed(address indexed controller, address indexed buyer, uint256 amount);
    event AllocationRestored(address indexed controller, address indexed buyer, uint256 amount);

    constructor() {
        _disableInitializers();
    }

    function initialize(address initialOwner) external initializer {
        require(initialOwner != address(0), "Owner required");
        __Ownable_init(initialOwner);
    }

    /**
     * @notice Define or update purchase limits for multiple buyers.
     * @param buyers Addresses to update.
     * @param maxAllocations Maximum BRLA amount each buyer may spend (18 decimals).
     * @param enabled Whether the allocation is active.
     */
    function setAllocations(address[] calldata buyers, uint256[] calldata maxAllocations, bool[] calldata enabled)
        external
        onlyController
    {
        uint256 length = buyers.length;
        require(length == maxAllocations.length && length == enabled.length, "Array length mismatch");
        for (uint256 i = 0; i < length; ++i) {
            address buyer = buyers[i];
            require(buyer != address(0), "Invalid buyer");
            Allocation storage allocation = allocations[buyer];
            allocation.maxAllocation = maxAllocations[i];
            allocation.enabled = enabled[i];
            emit AllocationUpdated(buyer, maxAllocations[i], enabled[i]);
        }
    }

    /**
     * @notice Reset consumed amount for provided buyers.
     * @param buyers Addresses whose purchased amount should be reset to zero.
     */
    function resetConsumed(address[] calldata buyers) external onlyController {
        for (uint256 i = 0; i < buyers.length; ++i) {
            Allocation storage allocation = allocations[buyers[i]];
            allocation.consumed = 0;
        }
    }

    /**
     * @notice Allow or block a sale contract from consuming allocations.
     * @param controller Sale contract address.
     * @param allowed Whether the controller can record purchases.
     */
    function setController(address controller, bool allowed) external onlyOwner {
        require(controller != address(0), "Invalid controller");
        controllers[controller] = allowed;
        emit ControllerUpdated(controller, allowed);
    }

    /**
     * @notice Allow controllers to add or remove other controllers.
     */
    function setControllerFromController(address controller, bool allowed) external onlyController {
        require(controller != address(0), "Invalid controller");

        controllers[controller] = allowed;
        emit ControllerUpdated(controller, allowed);
    }

    /**
     * @notice Check remaining allocation for a buyer.
     * @param buyer Address to query.
     * @return remaining BRLA the buyer can still spend.
     */
    function remainingAllocation(address buyer) external view returns (uint256 remaining) {
        Allocation memory allocation = allocations[buyer];
        if (!allocation.enabled || allocation.maxAllocation <= allocation.consumed) {
            return 0;
        }
        remaining = allocation.maxAllocation - allocation.consumed;
    }

    /**
     * @notice View helper indicating if a buyer can purchase a given amount.
     * @param buyer Address attempting to buy.
     * @param amount BRLA amount requested.
     * @return `true` when enough allocation remains and allowlist enabled.
     */
    function canPurchase(address buyer, uint256 amount) external view returns (bool) {
        Allocation memory allocation = allocations[buyer];
        if (!allocation.enabled) {
            return false;
        }
        return allocation.maxAllocation >= allocation.consumed + amount;
    }

    /**
     * @notice Record a purchase and consume allocation; callable by authorized sale contracts.
     * @param buyer Address purchasing offer tokens.
     * @param amount BRLA amount spent by the buyer.
     */
    function consumeAllocation(address buyer, uint256 amount) external onlyController {
        Allocation storage allocation = allocations[buyer];
        require(allocation.enabled, "Buyer not whitelisted");
        uint256 newConsumed = allocation.consumed + amount;
        require(newConsumed <= allocation.maxAllocation, "Allocation exceeded");
        allocation.consumed = newConsumed;
        emit AllocationConsumed(msg.sender, buyer, amount);
    }

    /**
     * @notice Return allocation when a sale rolls back a purchase (e.g. refund).
     * @param buyer Address whose consumed amount should decrease.
     * @param amount BRLA amount being returned to the allowlist.
     */
    function restoreAllocation(address buyer, uint256 amount) external onlyController {
        Allocation storage allocation = allocations[buyer];
        require(amount > 0, "Amount must be positive");
        require(allocation.consumed >= amount, "Restore exceeds consumed");
        allocation.consumed -= amount;
        emit AllocationRestored(msg.sender, buyer, amount);
    }

    /// @dev Required by UUPSUpgradeable — only the owner can authorize upgrades.
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}

    function version() external pure returns (string memory) {
        return "1.0.1";
    }

    /// @dev shrink the gap when adding next updates
    /// Do not add variables after the gap.
    /// Do not reorder existing ones.
    uint256[50] private __gap;
}
