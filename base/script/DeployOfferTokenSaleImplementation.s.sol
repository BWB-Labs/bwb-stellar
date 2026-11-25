// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferTokenSale} from "../src/OfferTokenSale.sol";

/**
 * @title DeployOfferTokenSaleImplementation
 * @notice Deploys the OfferTokenSale implementation contract and logs the address.
 */
contract DeployOfferTokenSaleImplementation is Script {
    function run() external returns (OfferTokenSale implementation) {
        vm.startBroadcast();
        implementation = new OfferTokenSale();
        vm.stopBroadcast();

        console2.log("OfferTokenSale implementation deployed at:", address(implementation));
    }
}
