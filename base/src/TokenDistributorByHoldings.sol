// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {IERC20Metadata} from "openzeppelin-contracts/contracts/token/ERC20/extensions/IERC20Metadata.sol";
import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";

/**
 * @title TokenDistributorByHoldings
 * @notice Implements a pull-based dividend model for payment token distributions. Any amount deposited
 *         through {distribute} is converted into a per-offer-token accumulator. Holders can
 *         claim their share on demand without requiring owner-supplied holder lists or batch
 *         processing, ensuring fair payouts regardless of the number of holders.
 * @custom:oz-upgrades-unsafe-allow constructor
 */
contract TokenDistributorByHoldings is Initializable, OwnableUpgradeable {
    using SafeERC20 for IERC20;

    /// @notice Fixed-point scaling factor derived from the payment token's decimals.
    uint256 private magnitude;

    /// @notice Payment token that is distributed.
    IERC20 public paymentToken;

    /// @notice Offer token used to measure proportional ownership.
    IERC20 public offerToken;

    /// @notice Cumulative payment tokens allocated per OfferToken (scaled by the payment token's decimals).
    uint256 public cumulativePerToken;

    /// @notice Tracks the cumulative value per holder that has already been credited.
    mapping(address => uint256) private creditedPerToken;

    /// @notice Emitted when the payment token is deposited and distributed to the accumulator.
    event DividendsDistributed(uint256 amount, uint256 cumulativePerToken);

    /// @notice Emitted when a holder claims their pending payment tokens.
    event DividendsClaimed(address indexed holder, uint256 amount);

    /// @notice Emitted when non-payment tokens are rescued by the owner.
    event TokensWithdrawn(address indexed token, address indexed recipient, uint256 amount);

    constructor() {
        _disableInitializers();
    }

    function __TokenDistributorByHoldings_init(address brlaToken_, address offerToken_, address owner_)
        internal
        onlyInitializing
    {
        require(brlaToken_ != address(0), "Invalid payment token address");
        require(offerToken_ != address(0), "Invalid Offer token address");
        require(owner_ != address(0), "Owner required");

        __Ownable_init(owner_);
        paymentToken = IERC20(brlaToken_);
        offerToken = IERC20(offerToken_);
        uint8 decimals = IERC20Metadata(brlaToken_).decimals();
        magnitude = 10 ** uint256(decimals);
    }

    function initialize(address brlaToken_, address offerToken_, address owner_) external virtual initializer {
        __TokenDistributorByHoldings_init(brlaToken_, offerToken_, owner_);
    }

    /**
     * @notice Deposit the payment token (via direct transfer from `payer`) and update the per-token accumulator.
     * @param payer Address providing the payment tokens.
     * @param amount Amount of payment tokens to allocate.
     */
    function distribute(address payer, uint256 amount) external onlyOwner {
        require(amount > 0, "Amount must be > 0");
        require(payer != address(0), "Invalid payer address");

        uint256 supply = offerToken.totalSupply();
        require(supply > 0, "Offer token supply is zero");

        paymentToken.safeTransferFrom(payer, address(this), amount);

        uint256 perTokenIncrement = (amount * magnitude) / supply;
        require(perTokenIncrement > 0, "Amount too small");

        cumulativePerToken += perTokenIncrement;
        emit DividendsDistributed(amount, cumulativePerToken);
    }

    /**
     * @notice Claim accrued payment tokens for the caller.
     */
    function claim() external {
        _claim(msg.sender);
    }

    /**
     * @notice Claim accrued payment tokens on behalf of another holder.
     * @param holder Address whose rewards should be claimed.
     */
    function claimFor(address holder) external {
        _claim(holder);
    }

    /**
     * @notice Claim accrued payment tokens for multiple holders.
     * @param holders Array of holder addresses to settle.
     */
    function claimMany(address[] calldata holders) external {
        uint256 length = holders.length;
        require(length > 0, "No holders provided");
        for (uint256 i = 0; i < length; ++i) {
            _claim(holders[i]);
        }
    }

    /**
     * @notice Return the currently claimable payment tokens for a holder.
     * @param holder Address to inspect.
     */
    function claimable(address holder) public view returns (uint256) {
        uint256 owed = (offerToken.balanceOf(holder) * cumulativePerToken) / magnitude;
        uint256 credited = creditedPerToken[holder];
        if (owed <= credited) {
            return 0;
        }
        return owed - credited;
    }

    function _claim(address holder) private {
        require(holder != address(0), "Holder cannot be zero address");

        uint256 balance = offerToken.balanceOf(holder);
        if (balance == 0) {
            return;
        }

        uint256 owed = (balance * cumulativePerToken) / magnitude;
        uint256 credited = creditedPerToken[holder];
        if (owed <= credited) {
            return;
        }

        uint256 payout = owed - credited;
        creditedPerToken[holder] = owed;
        paymentToken.safeTransfer(holder, payout);
        emit DividendsClaimed(holder, payout);
    }

    /**
     * @notice Withdraw any ERC20 tokens left in the contract.
     * @param token Address of the ERC20 to withdraw (payment token or other).
     * @param recipient Address receiving the withdrawn tokens.
     */
    function withdrawTokens(address token, address recipient) external onlyOwner {
        require(token != address(0), "Invalid token address");
        require(recipient != address(0), "Invalid recipient address");
        uint256 amount = IERC20(token).balanceOf(address(this));
        require(amount > 0, "No tokens to withdraw");
        IERC20(token).safeTransfer(recipient, amount);
        emit TokensWithdrawn(token, recipient, amount);
    }

    function version() external pure returns (string memory) {
        return "1.0.0";
    }

    /// @dev shrink the gap when adding next updates
    /// Do not add variables after the gap.
    /// Do not reorder existing ones.
    uint256[50] private __gap;
}
