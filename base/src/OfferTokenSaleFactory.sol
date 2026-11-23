// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {UpgradeableBeacon} from "@openzeppelin/contracts/proxy/beacon/UpgradeableBeacon.sol";
import {BeaconProxy} from "@openzeppelin/contracts/proxy/beacon/BeaconProxy.sol";

import {OfferTokenSale} from "./OfferTokenSale.sol";

/**
 * @title OfferTokenSaleFactory
 * @notice Deploys OfferTokenSale contracts and emits an event for each new sale.
 */
contract OfferTokenSaleFactory is Ownable {
    UpgradeableBeacon public immutable beacon;

    error NotController(address caller);
    error ZeroAddressController();

    /// @notice Emitted whenever a new OfferTokenSale contract is deployed.
    event OfferTokenSaleDeployed(
        address indexed sale,
        address indexed offerToken,
        address indexed owner,
        address brlaToken,
        uint256 brlaPerToken,
        uint256 totalTokensForSale,
        address allowlist
    );

    event BeaconUpgraded(address indexed newImplementation);

    event ControllerSet(address indexed controller, bool allowed);

    mapping(address => bool) private controllers;

    modifier onlyController() {
        if (!controllers[_msgSender()]) {
            revert NotController(_msgSender());
        }
        _;
    }

    constructor(address initialOwner, address initialImplementation) Ownable(initialOwner) {
        beacon = new UpgradeableBeacon(initialImplementation, address(this));
        controllers[initialOwner] = true;
        emit ControllerSet(initialOwner, true);
    }

    function upgradeImplementation(address newImplementation) external onlyOwner {
        beacon.upgradeTo(newImplementation);
        emit BeaconUpgraded(newImplementation);
    }

    function setController(address controller, bool allowed) external onlyOwner {
        if (controller == address(0)) {
            revert ZeroAddressController();
        }
        controllers[controller] = allowed;
        emit ControllerSet(controller, allowed);
    }

    function isController(address controller) external view returns (bool) {
        return controllers[controller];
    }

    /**
     * @notice Deploy a new OfferTokenSale contract.
     * @param offerToken Address of the Offer ERC20 token being sold.
     * @param brlaToken Address of the BRLA payment token.
     * @param brlaPerToken Price in BRLA (18 decimals) required for one offer token.
     * @param allowlist Address of the shared OfferSaleAllowlist contract.
     * @param initialOwner Address receiving ownership of the sale contract.
     * @return sale Address of the deployed OfferTokenSale contract.
     */
    function createOfferTokenSale(
        address offerToken,
        address brlaToken,
        uint256 brlaPerToken,
        address allowlist,
        address initialOwner,
        OfferTokenSale.SaleConfig calldata saleConfig
    ) external onlyController returns (address sale) {
        require(offerToken != address(0), "Offer token required");
        require(brlaToken != address(0), "BRLA token required");
        require(allowlist != address(0), "Allowlist required");
        require(initialOwner != address(0), "Owner required");
        require(brlaPerToken > 0, "Price must be positive");
        require(saleConfig.hardCap >= brlaPerToken, "Hard cap below one token");
        require(saleConfig.coolingOffPeriod == 0 || saleConfig.coolingOffPeriod >= 1 minutes, "Cooling-off too short");
        require(saleConfig.minSuccessPercent == 0 || saleConfig.minSuccessPercent >= 10, "Percent below minimum");

        bytes memory initData = abi.encodeCall(
            OfferTokenSale.initialize, (offerToken, brlaToken, brlaPerToken, allowlist, initialOwner, saleConfig)
        );

        BeaconProxy proxy = new BeaconProxy(address(beacon), initData);
        sale = address(proxy);
        uint256 totalTokensForSale = OfferTokenSale(sale).totalTokensForSale();

        emit OfferTokenSaleDeployed(
            sale, offerToken, initialOwner, brlaToken, brlaPerToken, totalTokensForSale, allowlist
        );
    }
}
