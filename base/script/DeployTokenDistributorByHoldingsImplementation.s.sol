// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {TokenDistributorByHoldings} from "../src/TokenDistributorByHoldings.sol";

/**
 * @title DeployTokenDistributorByHoldingsImplementation
 * @notice Deploys the TokenDistributorByHoldings implementation contract and logs the address.
 */
contract DeployTokenDistributorByHoldingsImplementation is Script {
    function run() external returns (TokenDistributorByHoldings implementation) {
        vm.startBroadcast();
        implementation = new TokenDistributorByHoldings();
        vm.stopBroadcast();

        console2.log("TokenDistributorByHoldings implementation deployed at:", address(implementation));
    }
}
