// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferSaleAllowlist} from "../src/OfferSaleAllowlist.sol";

/**
 * @title ConfigureOfferSaleAllowlist
 * @notice Foundry script that batch-updates allowlist allocations using JSON input.
 */
contract ConfigureOfferSaleAllowlist is Script {
    struct ControllerParams {
        address allowlistAddress;
        address controller;
        bool enabled;
    }

    /**
     * @notice Updates the allowlist allocations for the provided buyers.
     */
    function run() external {
        ControllerParams memory params = _loadParams();
        OfferSaleAllowlist allowlist = OfferSaleAllowlist(params.allowlistAddress);

        console2.log("Configuring controller:");
        console2.log("Allowlist:", params.allowlistAddress);
        console2.log("Controller:", params.controller);
        console2.log("Enabled:", params.enabled);

        vm.startBroadcast();
        allowlist.setController(params.controller, params.enabled);
        vm.stopBroadcast();

        console2.log("Controller updated successfully");
    }

    function _loadParams() internal view returns (ControllerParams memory params) {
        params.allowlistAddress = vm.envAddress("ALLOWLIST_ADDRESS");
        params.controller = vm.envAddress("CONTROLLER_ADDRESS");
        params.enabled = vm.envBool("CONTROLLER_ENABLED");
    }

    // Removed JSON loader

    // Removed preview logger
}
