// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {UpgradeableBeacon} from "@openzeppelin/contracts/proxy/beacon/UpgradeableBeacon.sol";
import {BeaconProxy} from "@openzeppelin/contracts/proxy/beacon/BeaconProxy.sol";

import {OfferERC20} from "./OfferERC20.sol";

/**
 * @title OfferTokenFactory
 * @notice Deploys upgradeable OfferERC20 tokens with configurable metadata.
 */
contract OfferTokenFactory is Ownable {
    UpgradeableBeacon public immutable beacon;

    error NotController(address caller);
    error ZeroAddressController();

    /// @notice Emitted when a new OfferERC20 token is deployed
    event OfferTokenDeployed(address indexed token, address indexed owner, string name, string symbol, string offerURI);

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
     * @notice Deploy a new OfferERC20 token
     * @param name_ ERC20 token name
     * @param symbol_ ERC20 token symbol
     * @param owner_ Address receiving ownership and initial supply
     * @param initialSupply Total supply (in token units) minted to the owner
     * @param offerURI_ Metadata URI describing the offer details
     * @return token Address of the newly deployed OfferERC20 token
     */
    function createOfferToken(
        string calldata name_,
        string calldata symbol_,
        address owner_,
        uint256 initialSupply,
        string calldata offerURI_,
        address transferAdmin_
    ) external onlyController returns (address token) {
        require(owner_ != address(0), "Owner cannot be zero address");
        require(transferAdmin_ != address(0), "Transfer admin required");

        bytes memory initData = abi.encodeCall(
            OfferERC20.initialize, (name_, symbol_, owner_, initialSupply, offerURI_, transferAdmin_, new address[](0))
        );

        BeaconProxy proxy = new BeaconProxy(address(beacon), initData);
        token = address(proxy);
        emit OfferTokenDeployed(token, owner_, name_, symbol_, offerURI_);
    }
}
