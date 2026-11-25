// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferSaleAllowlist} from "../src/OfferSaleAllowlist.sol";

/**
 * @title DeployOfferSaleAllowlistImplementation
 * @notice Deploys the OfferSaleAllowlist implementation contract and logs the address.
 */
contract DeployOfferSaleAllowlistImplementation is Script {
    function run() external returns (OfferSaleAllowlist implementation) {
        vm.startBroadcast();
        implementation = new OfferSaleAllowlist();
        vm.stopBroadcast();

        console2.log("OfferSaleAllowlist implementation deployed at:", address(implementation));
    }
}
