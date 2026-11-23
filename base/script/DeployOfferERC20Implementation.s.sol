// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferERC20} from "../src/OfferERC20.sol";

/**
 * @title DeployOfferERC20Implementation
 * @notice Deploys the OfferERC20 implementation contract and logs the address.
 */
contract DeployOfferERC20Implementation is Script {
    function run() external returns (OfferERC20 implementation) {
        address safe = vm.envAddress("SAFE_ADDRESS");
        address transferAdmin = _loadTransferAdminOrDefault(safe);

        vm.startBroadcast();
        implementation = new OfferERC20();
        vm.stopBroadcast();

        console2.log("OfferERC20 implementation deployed at:", address(implementation));
        console2.log("Transfer admin (use during initialization):", transferAdmin);
    }

    function _loadTransferAdminOrDefault(address fallbackAdmin) internal view returns (address) {
        try vm.envAddress("TRANSFER_ADMIN_ADDRESS") returns (address provided) {
            return provided;
        } catch {
            return fallbackAdmin;
        }
    }
}
