// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferERC20} from "../src/OfferERC20.sol";
import {OfferTokenFactory} from "../src/OfferTokenFactory.sol";

/**
 * @title DeployOfferTokenFactory
 * @notice Foundry script that deploys the OfferTokenFactory contract
 */
contract DeployOfferTokenFactory is Script {
    function run() external returns (OfferTokenFactory factory) {
        vm.startBroadcast();

        (, address broadcaster,) = vm.readCallers();
        address owner = broadcaster;
        address transferAdmin = _loadTransferAdminOrDefault(owner);
        OfferERC20 implementation = new OfferERC20();
        factory = new OfferTokenFactory(owner, address(implementation));

        vm.stopBroadcast();

        console2.log("Deployer (tx.origin):", owner);
        console2.log("OfferTokenFactory deployed at:", address(factory));
        console2.log("OfferERC20 implementation deployed at:", address(implementation));
        console2.log("Owner assigned:", owner);
        console2.log("Transfer admin (supply to createOfferToken):", transferAdmin);
    }

    function _loadTransferAdminOrDefault(address fallbackAdmin) internal view returns (address) {
        try vm.envAddress("TRANSFER_ADMIN_ADDRESS") returns (address provided) {
            return provided;
        } catch {
            return fallbackAdmin;
        }
    }
}
