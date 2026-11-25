// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {TokenDistributor} from "../src/TokenDistributor.sol";

/**
 * @title DeployTokenDistributorImplementation
 * @notice Deploys the TokenDistributor implementation contract and logs the address.
 */
contract DeployTokenDistributorImplementation is Script {
    function run() external returns (TokenDistributor implementation) {
        vm.startBroadcast();
        implementation = new TokenDistributor();
        vm.stopBroadcast();

        console2.log("TokenDistributor implementation deployed at:", address(implementation));
    }
}
