// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/proxy/beacon/UpgradeableBeacon.sol";
import "@openzeppelin/contracts/proxy/beacon/BeaconProxy.sol";
import "./TokenDistributor.sol";

contract TokenDistributorFactory is Ownable {
    UpgradeableBeacon public immutable beacon;

    error NotController(address caller);
    error ZeroAddressController();

    event TokenDistributorCreated(address indexed proxy, address indexed owner, string name, string descriptionURI);

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

    function createTokenDistributor(address owner_, string calldata _name, string calldata _descriptionURI)
        external
        onlyController
        returns (address proxy)
    {
        bytes memory initData =
            abi.encodeWithSelector(TokenDistributor.initialize.selector, owner_, _name, _descriptionURI);

        BeaconProxy p = new BeaconProxy(address(beacon), initData);
        proxy = address(p);

        emit TokenDistributorCreated(proxy, owner_, _name, _descriptionURI);
    }
}
