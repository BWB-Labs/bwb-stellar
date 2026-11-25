// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {ReentrancyGuardTransient} from "@openzeppelin/contracts/utils/ReentrancyGuardTransient.sol";
import {OfferSaleAllowlist} from "./OfferSaleAllowlist.sol";

/**
 * @title OfferTokenSale
 * @notice Allows investors to reserve Offer ERC20 tokens by paying BRLA.
 *         Tokens are delivered only after the entire allocation is sold and the
 *         sale is finalized. The owner can also cancel the sale and enable
 *         refunds that return BRLA to every buyer.
 *
 *
 * @dev the ReentrancyGuardTransient only works for chains that implement the EIP-1153.
 * Keep that in mind when choosing a network to deploy this contract. In the case of the
 * main choice for network of this project, BASE does implement EIP-1153
 * @custom:oz-upgrades-unsafe-allow constructor
 */
contract OfferTokenSale is Initializable, OwnableUpgradeable, ReentrancyGuardTransient {
    using SafeERC20 for IERC20;

    uint256 private constant PERCENT_SCALE = 100;
    uint256 private constant DEFAULT_MIN_SUCCESS_PERCENT = 75;
    uint256 private constant DEFAULT_COOLING_OFF_PERIOD = 5 days;

    /// @notice Lifecycle states defined for CVM 88 compliance
    enum SaleState {
        Preparing,
        Active,
        Successful,
        Failed
    }

    /// @notice Offer ERC20 token distributed by this sale
    IERC20 public offerToken;

    /// @notice BRLA token used as the payment currency
    IERC20 public brlaToken;

    /// @notice BRLA required (18 decimals) to purchase exactly one offer token
    uint256 public brlaPerToken;

    /// @notice Global allowlist contract gating who can purchase offer tokens
    OfferSaleAllowlist public allowlist;

    /// @notice Total number of offer tokens being sold in this sale (entire token supply)
    uint256 public totalTokensForSale;

    /// @notice Aggregate amount of offer tokens reserved by buyers
    uint256 public soldTokens;

    /// @notice True once all tokens are sold and the sale is finalized (legacy flag)
    bool public isFinalized;

    /// @notice True if the owner has cancelled the sale and enabled refunds (legacy flag)
    bool public refundsEnabled;

    /// @notice True after the inventory provider has transferred the entire token supply into the contract
    bool public inventorySeeded;

    /// @notice Address that supplies tokens to seed the sale inventory
    address public inventoryProvider;

    /// @notice Tracks the overall sale lifecycle
    SaleState public saleState; // appended — no byte offset shift

    /// @notice Maximum BRLA the sale can raise
    uint256 public hardCap;

    /// @notice Cooling-off window investors have to request refunds
    uint256 public coolingOffPeriod;

    /// @notice Minimum percentage of tokens that must be sold to finalize successfully
    uint256 public minSuccessPercent;

    /// @notice Aggregate BRLA raised (net of refunds)
    uint256 public totalBrlaRaised;

    struct SaleConfig {
        uint256 hardCap;
        uint256 coolingOffPeriod;
        uint256 minSuccessPercent;
    }

    struct BuyerPosition {
        uint256 tokens;
        uint256 brla;
        uint256 lastPurchaseAt;
    }

    mapping(address => BuyerPosition) private buyerPositions;

    /// @notice Emitted when a buyer swaps BRLA for Offer tokens (reservation only)
    event TokensPurchased(address indexed buyer, uint256 brlaSpent, uint256 tokensBought);

    /// @notice Emitted when the owner seeds the sale with the full token inventory
    event InventorySeeded(uint256 amount);

    /// @notice Emitted when the inventory provider address changes
    event InventoryProviderUpdated(address indexed newProvider);

    /// @notice Emitted when the sale is finalized after selling out
    event SaleFinalized(uint256 totalTokensSold);

    /// @notice Emitted when refunds are enabled by the owner
    event RefundsEnabled();

    /// @notice Emitted when a buyer receives their offer tokens after finalization
    event TokensClaimed(address indexed buyer, uint256 amount);

    /// @notice Emitted when a buyer reclaims their BRLA after refunds are enabled
    event RefundClaimed(address indexed buyer, uint256 brlaAmount, uint256 tokensCancelled);

    /// @notice emmited when the state of the offerSale changes
    event SaleStateChanged(SaleState previousState, SaleState newState);

    function tokensPurchased(address buyer) external view returns (uint256) {
        return buyerPositions[buyer].tokens;
    }

    function brlaSpent(address buyer) external view returns (uint256) {
        return buyerPositions[buyer].brla;
    }

    function lastPurchaseTimestamp(address buyer) external view returns (uint256) {
        return buyerPositions[buyer].lastPurchaseAt;
    }

    constructor() {
        _disableInitializers();
    }

    function initialize(
        address offerToken_,
        address brlaToken_,
        uint256 brlaPerToken_,
        address allowlist_,
        address owner_,
        SaleConfig calldata saleConfig
    ) external initializer {
        require(offerToken_ != address(0), "Offer token required");
        require(brlaToken_ != address(0), "BRLA token required");
        require(allowlist_ != address(0), "Allowlist required");
        require(owner_ != address(0), "Owner required");
        require(brlaPerToken_ > 0, "Price must be positive");
        require(saleConfig.hardCap > 0, "Hard cap required");

        __Ownable_init(owner_);

        IERC20 offerTokenInstance = IERC20(offerToken_);
        uint256 supplyForSale = offerTokenInstance.totalSupply();
        require(supplyForSale > 0, "Offer token supply zero");

        offerToken = offerTokenInstance;
        brlaToken = IERC20(brlaToken_);
        brlaPerToken = brlaPerToken_;
        allowlist = OfferSaleAllowlist(allowlist_);
        totalTokensForSale = supplyForSale;
        inventoryProvider = owner_;
        hardCap = saleConfig.hardCap;
        coolingOffPeriod = saleConfig.coolingOffPeriod == 0 ? DEFAULT_COOLING_OFF_PERIOD : saleConfig.coolingOffPeriod;
        uint256 percent = saleConfig.minSuccessPercent == 0 ? DEFAULT_MIN_SUCCESS_PERCENT : saleConfig.minSuccessPercent;
        require(percent <= PERCENT_SCALE, "Percent above 100");
        minSuccessPercent = percent;
        saleState = SaleState.Preparing;
        totalBrlaRaised = 0;
        isFinalized = false;
        refundsEnabled = false;
        inventorySeeded = false;
    }

    /**
     * @notice Seed the sale with the entire offer token supply so buyers can start reserving.
     *         Pulls tokens from the configured inventory provider after they grant allowance.
     */
    function seedInventory() external onlyOwner nonReentrant {
        require(saleState == SaleState.Preparing, "Sale not preparing");
        require(!inventorySeeded, "Inventory seeded");
        address provider = inventoryProvider;
        require(provider != address(0), "Inventory provider missing");
        uint256 amount = totalTokensForSale;
        offerToken.safeTransferFrom(provider, address(this), amount);
        require(offerToken.balanceOf(address(this)) >= amount, "Inventory incomplete");
        inventorySeeded = true;
        _setSaleState(SaleState.Active);
        emit InventorySeeded(amount);
    }

    /**
     * @notice Update the wallet that supplies tokens during the seeding process.
     *         Can only be changed before inventory is seeded.
     * @param newProvider Address that holds the offer token supply.
     */
    function updateInventoryProvider(address newProvider) external onlyOwner {
        require(!inventorySeeded, "Inventory seeded");
        require(newProvider != address(0), "Provider required");
        inventoryProvider = newProvider;
        emit InventoryProviderUpdated(newProvider);
    }

    /**
     * @notice Reserve offer tokens by paying BRLA. Tokens are claimable only after
     *         the sale is finalized.
     * @param brlaAmount Amount of BRLA (18 decimals) to spend.
     * @return tokensBought Offer tokens reserved for the buyer.
     */
    function buyWithBrla(uint256 brlaAmount) external nonReentrant returns (uint256 tokensBought) {
        require(inventorySeeded, "Inventory not seeded");
        require(saleState == SaleState.Active, "Sale not active");
        require(brlaAmount > 0, "BRLA amount is zero");

        // Measure actual BRLA received (to support fee-on-transfer tokens)
        uint256 balanceBefore = brlaToken.balanceOf(address(this));
        // Note: still attempt to transfer the requested BRLA amount; if the
        // token takes a fee, the contract will receive less than brlaAmount.
        brlaToken.safeTransferFrom(msg.sender, address(this), brlaAmount);
        uint256 balanceAfter = brlaToken.balanceOf(address(this));
        uint256 actualBRLAReceived = balanceAfter - balanceBefore;
        require(actualBRLAReceived > 0, "Insufficient BRLA received");
        require(actualBRLAReceived % brlaPerToken == 0, "BRLA must match whole tokens");
        tokensBought = actualBRLAReceived / brlaPerToken;
        require(tokensBought > 0, "Insufficient BRLA for tokens");
        require(soldTokens + tokensBought <= totalTokensForSale, "Exceeds allocation");
        require(totalBrlaRaised + actualBRLAReceived <= hardCap, "Hard cap exceeded");

        // Update allowlist and internal accounting using the actual received BRLA
        allowlist.consumeAllocation(msg.sender, actualBRLAReceived);

        BuyerPosition storage position = buyerPositions[msg.sender];
        position.brla += actualBRLAReceived;
        position.tokens += tokensBought;
        position.lastPurchaseAt = block.timestamp;
        totalBrlaRaised += actualBRLAReceived;
        soldTokens += tokensBought;

        emit TokensPurchased(msg.sender, actualBRLAReceived, tokensBought);
    }

    /**
     * @notice Preview how many offer tokens will be received for a BRLA amount.
     * @param brlaAmount Amount of BRLA to spend (18 decimals).
     * @return Offer tokens dispensed for the provided BRLA amount.
     */
    function previewTokens(uint256 brlaAmount) external view returns (uint256) {
        return _convertBrlaToTokens(brlaAmount);
    }

    /**
     * @notice Finalize the sale once every offer token has been sold.
     *         Requires the contract to hold the entire allocation for delivery.
     */
    function finalizeSale() external onlyOwner nonReentrant {
        require(inventorySeeded, "Inventory not seeded");
        require(saleState == SaleState.Active, "Sale not active");
        require(_hasMetSuccessCriteria() || soldTokens == totalTokensForSale, "Success criteria unmet");
        require(offerToken.balanceOf(address(this)) >= soldTokens, "Insufficient offer balance");

        isFinalized = true;
        refundsEnabled = false;
        _setSaleState(SaleState.Successful);

        emit SaleFinalized(soldTokens);
    }

    /**
     * @notice Mark the sale as failed when the success criteria has not been met.
     */
    function markSaleFailed() external onlyOwner {
        require(saleState == SaleState.Active, "Sale not active");
        require(!_hasMetSuccessCriteria(), "Success criteria met");
        refundsEnabled = true;
        isFinalized = false;
        _setSaleState(SaleState.Failed);
        emit RefundsEnabled();
    }

    /**
     * @notice Batch helper that releases tokens for multiple buyers after finalization.
     * @param recipients List of buyers to receive their tokens.
     */
    function releaseTokens(address[] calldata recipients) external nonReentrant {
        require(saleState == SaleState.Successful, "Sale not successful");
        uint256 length = recipients.length;
        for (uint256 i = 0; i < length; ++i) {
            _releaseTokens(recipients[i]);
        }
    }

    /**
     * @notice Enable refunds, cancelling the sale and allowing each buyer to reclaim their BRLA.
     *         Refunds can only be enabled before the sale is finalized.
     */
    function enableRefunds() external onlyOwner {
        require(saleState == SaleState.Active, "Sale not active");
        require(!refundsEnabled, "Refunds active");
        refundsEnabled = true;
        isFinalized = false;
        _setSaleState(SaleState.Failed);
        emit RefundsEnabled();
    }

    /**
     * @notice Batch helper that processes refunds for multiple buyers.
     * @param recipients List of buyers to refund.
     */
    function processRefunds(address[] calldata recipients) external nonReentrant {
        require(refundsEnabled && saleState == SaleState.Failed, "Refunds disabled");
        uint256 length = recipients.length;
        for (uint256 i = 0; i < length; ++i) {
            _refundBuyer(recipients[i]);
        }
    }

    /**
     * @notice Allows buyers to exercise their 5-day cooling-off right while the sale is active or has just concluded successfully.
     *         Even after the owner finalizes the sale, buyers retain this right for five days after their last purchase.
     * @return refunded Amount of BRLA returned to the caller.
     */
    function requestCoolingOffRefund() external nonReentrant returns (uint256 refunded) {
        bool coolingOffOpen = saleState == SaleState.Active || saleState == SaleState.Successful;
        require(coolingOffOpen, "Cooling-off closed");
        BuyerPosition storage position = buyerPositions[msg.sender];
        uint256 lastPurchase = position.lastPurchaseAt;
        require(lastPurchase != 0, "Cooling-off unavailable");
        require(block.timestamp <= lastPurchase + coolingOffPeriod, "Cooling-off expired");
        refunded = _refundBuyer(msg.sender);
        require(refunded > 0, "Nothing to refund");
    }

    /**
     * @notice Withdraw collected BRLA to a destination wallet after the sale is finalized.
     * @param to Recipient address.
     * @param amount Amount of BRLA to withdraw.
     */
    function withdrawBrla(address to, uint256 amount) external onlyOwner {
        require(to != address(0), "Invalid recipient");
        require(saleState == SaleState.Successful, "Sale not successful");
        brlaToken.safeTransfer(to, amount);
    }

    /**
     * @notice Recover remaining Offer tokens back to the owner once refunds are enabled.
     * @param to Recipient address.
     * @param amount Amount of Offer tokens to withdraw.
     */
    function withdrawOfferTokens(address to, uint256 amount) external onlyOwner {
        require(to != address(0), "Invalid recipient");
        require(saleState == SaleState.Failed, "Sale not failed");
        offerToken.safeTransfer(to, amount);
    }

    function _convertBrlaToTokens(uint256 brlaAmount) private view returns (uint256) {
        require(brlaAmount % brlaPerToken == 0, "BRLA must match whole tokens");
        return brlaAmount / brlaPerToken;
    }

    function _hasMetSuccessCriteria() private view returns (bool) {
        return soldTokens * PERCENT_SCALE >= totalTokensForSale * minSuccessPercent;
    }

    function _setSaleState(SaleState newState) private {
        if (saleState == newState) {
            return;
        }
        SaleState previous = saleState;
        saleState = newState;
        emit SaleStateChanged(previous, newState);
    }

    function _releaseTokens(address buyer) private returns (uint256 amount) {
        BuyerPosition storage position = buyerPositions[buyer];
        amount = position.tokens;
        if (amount == 0) {
            return 0;
        }

        position.tokens = 0;
        position.brla = 0;
        position.lastPurchaseAt = 0;

        offerToken.safeTransfer(buyer, amount);
        emit TokensClaimed(buyer, amount);
    }

    function _refundBuyer(address buyer) private returns (uint256 refunded) {
        BuyerPosition storage position = buyerPositions[buyer];
        refunded = position.brla;
        if (refunded == 0) {
            return 0;
        }

        uint256 tokensCancelled = position.tokens;

        position.brla = 0;
        position.tokens = 0;
        position.lastPurchaseAt = 0;

        if (refunded > 0) {
            soldTokens -= tokensCancelled;
            allowlist.restoreAllocation(buyer, refunded);
        }

        totalBrlaRaised -= refunded;
        brlaToken.safeTransfer(buyer, refunded);
        emit RefundClaimed(buyer, refunded, tokensCancelled);
    }

    function version() external pure returns (string memory) {
        return "1.0.0";
    }

    /// @dev shrink the gap when adding next updates
    /// Do not add variables after the gap.
    /// Do not reorder existing ones.
    uint256[50] private __gap;
}
