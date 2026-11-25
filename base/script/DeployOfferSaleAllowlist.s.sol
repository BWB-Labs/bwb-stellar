// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {ERC1967Proxy} from "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {OfferSaleAllowlist} from "../src/OfferSaleAllowlist.sol";

/**
 * @title DeployOfferSaleAllowlist
 * @notice Foundry script that deploys the OfferSaleAllowlist contract using
 *         parameters sourced from environment variables.
 */
contract DeployOfferSaleAllowlist is Script {
    /**
     * @notice Deploy a new OfferSaleAllowlist instance
     * @return allowlist The deployed OfferSaleAllowlist contract
     */
    function run() external returns (OfferSaleAllowlist allowlist) {
        vm.startBroadcast();

        (, address broadcaster,) = vm.readCallers();
        address deployer = broadcaster;
        address owner = _loadAddressOrDefault("ALLOWLIST_OWNER", deployer);
        require(owner == deployer, "Owner override must match deployer");

        console2.log("Deployer (tx.origin):", deployer);
        console2.log("Owner assigned:", owner);

        address implementationAddress = _loadAddressOrZero("ALLOWLIST_IMPLEMENTATION_ADDRESS");
        if (implementationAddress == address(0)) {
            implementationAddress = address(new OfferSaleAllowlist());
        }

        bytes memory initData = abi.encodeCall(OfferSaleAllowlist.initialize, (owner));
        allowlist = OfferSaleAllowlist(address(new ERC1967Proxy(implementationAddress, initData)));

        allowlist.setController(deployer, true);
        console2.log("Controller assigned (deployer):", deployer);

        vm.stopBroadcast();

        console2.log("OfferSaleAllowlist deployed at:", address(allowlist));
        console2.log("OfferSaleAllowlist implementation at:", implementationAddress);
    }

    function _loadAddressOrZero(string memory key) internal view returns (address value) {
        try vm.envAddress(key) returns (address provided) {
            value = provided;
        } catch {
            value = address(0);
        }
    }

    function _loadAddressOrDefault(string memory key, address defaultAddress) internal view returns (address value) {
        address supplied = _loadAddressOrZero(key);
        if (supplied != address(0)) {
            return supplied;
        }
        return defaultAddress;
    }
}
