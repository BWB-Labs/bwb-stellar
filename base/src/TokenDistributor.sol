// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IERC20, SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";

/**
 * @title TokenDistributor
 * @notice Owner-controlled ERC20 distributor with optional metadata.
 *         Supports upgradeable deployments via proxy initializers.
 * @custom:oz-upgrades-unsafe-allow constructor
 */
contract TokenDistributor is Initializable, OwnableUpgradeable {
    using SafeERC20 for IERC20;

    /// @notice Human-readable distributor name
    string public name;

    /// @notice URI with distributor description/details (e.g., IPFS/HTTPS)
    string public descriptionURI;

    /// @notice Emitted when metadata is updated
    event MetadataUpdated(string name, string descriptionURI);

    /// @notice Emitted when a proxy instance is initialized
    event Initialized(address owner, string name, string descriptionURI);

    /// @notice Emitted after a batch token distribution
    event TokensDistributed(address indexed token, uint256 totalAmount, uint256 recipientCount);

    /// @notice Emitted for each recipient transfer, including the contract balance after the transfer
    event RecipientTokensDistributed(
        address indexed token, address indexed recipient, uint256 amount, uint256 distributorBalance
    );

    constructor() {
        _disableInitializers();
    }

    /**
     * @notice Initialize proxy instance
     * @param owner_ Owner/admin address for this instance
     * @param name_ Human-readable name
     * @param descriptionURI_ Description/details URI (IPFS/HTTPS)
     */
    function __TokenDistributor_init(address owner_, string calldata name_, string calldata descriptionURI_)
        internal
        onlyInitializing
    {
        require(owner_ != address(0), "Owner cannot be zero address");

        __Ownable_init(owner_);

        name = name_;
        descriptionURI = descriptionURI_;

        emit Initialized(owner_, name_, descriptionURI_);
    }

    function initialize(address owner_, string calldata name_, string calldata descriptionURI_)
        external
        virtual
        initializer
    {
        __TokenDistributor_init(owner_, name_, descriptionURI_);
    }

    /**
     * @notice Update distributor metadata
     * @param name_ New human-readable name
     * @param descriptionURI_ New description/details URI
     */
    function updateMetadata(string calldata name_, string calldata descriptionURI_) external onlyOwner {
        name = name_;
        descriptionURI = descriptionURI_;
        emit MetadataUpdated(name_, descriptionURI_);
    }

    function distributeTokens(address tokenAddress, address[] calldata recipients, uint256[] calldata amounts)
        external
        onlyOwner
    {
        require(tokenAddress != address(0), "Invalid token address");
        require(recipients.length == amounts.length, "Recipients and amounts mismatch");
        require(recipients.length <= 200, "Batch size exceeds limit");

        IERC20 token = IERC20(tokenAddress);

        uint256 totalAmount = 0;
        for (uint256 i = 0; i < recipients.length; ++i) {
            require(recipients[i] != address(0), "Cannot transfer to zero address");
            totalAmount += amounts[i];
        }

        uint256 distributorBalance = token.balanceOf(address(this));
        require(distributorBalance >= totalAmount, "Insufficient token balance");

        for (uint256 i = 0; i < recipients.length; ++i) {
            token.safeTransfer(recipients[i], amounts[i]);
            distributorBalance -= amounts[i];
            emit RecipientTokensDistributed(tokenAddress, recipients[i], amounts[i], distributorBalance);
        }

        emit TokensDistributed(tokenAddress, totalAmount, recipients.length);
    }

    function version() external pure returns (string memory) {
        return "1.0.0";
    }

    /// @dev shrink the gap when adding next updates
    /// Do not add variables after the gap.
    /// Do not reorder existing ones.
    uint256[50] private __gap;
}
