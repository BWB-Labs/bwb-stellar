// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {SafeAddressRegistry} from "../src/SafeAddressRegistry.sol";

/// @title DeploySafeAddressRegistry
/// @notice Deploys the SafeAddressRegistry contract, defaulting the owner to the
///         broadcasting account when GNOSIS_SAFE_ADDRESS is unset.
contract DeploySafeAddressRegistry is Script {
    function run() external returns (SafeAddressRegistry registry) {
        vm.startBroadcast();

        address safeOwner = _loadSafeAddressOrDefault(msg.sender);
        registry = new SafeAddressRegistry(safeOwner);

        vm.stopBroadcast();

        console2.log("SafeAddressRegistry deployed at:", address(registry));
        console2.log("Registry owner (Safe):", safeOwner);
    }

    function _loadSafeAddressOrDefault(address fallbackOwner) internal view returns (address) {
        try vm.envAddress("GNOSIS_SAFE_ADDRESS") returns (address provided) {
            return provided;
        } catch {
            return fallbackOwner;
        }
    }
}
