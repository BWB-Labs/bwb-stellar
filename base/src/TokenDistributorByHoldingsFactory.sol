// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/proxy/beacon/UpgradeableBeacon.sol";
import "@openzeppelin/contracts/proxy/beacon/BeaconProxy.sol";

import {TokenDistributorByHoldings} from "./TokenDistributorByHoldings.sol";

/**
 * @title TokenDistributorByHoldingsFactory
 * @notice Deploys upgradeable TokenDistributorByHoldings instances configured for payment + offer tokens.
 */
contract TokenDistributorByHoldingsFactory is Ownable {
    UpgradeableBeacon public immutable beacon;

    error NotController(address caller);
    error ZeroAddressController();

    event TokenDistributorByHoldingsCreated(
        address indexed distributor, address indexed owner, address indexed brlaToken, address offerToken
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

    function createDistributor(address owner, address brlaToken, address offerToken)
        external
        onlyController
        returns (address distributor)
    {
        require(owner != address(0), "Owner required");
        bytes memory initData =
            abi.encodeWithSelector(TokenDistributorByHoldings.initialize.selector, brlaToken, offerToken, owner);

        BeaconProxy proxy = new BeaconProxy(address(beacon), initData);
        distributor = address(proxy);
        emit TokenDistributorByHoldingsCreated(distributor, owner, brlaToken, offerToken);
    }
}
