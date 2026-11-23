// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {TokenDistributorFactory} from "../src/TokenDistributorFactory.sol";
import {TokenDistributor} from "../src/TokenDistributor.sol";

/**
 * @title DeployTokenDistributorFactory
 * @notice Foundry script that deploys the TokenDistributorFactory contract.
 */
contract DeployTokenDistributorFactory is Script {
    /**
     * @notice Deploys TokenDistributorFactory and logs the implementation + factory addresses.
     * @return factory The deployed TokenDistributorFactory instance.
     */
    function run() external returns (TokenDistributorFactory factory) {
        vm.startBroadcast();

        (, address broadcaster,) = vm.readCallers();
        address owner = broadcaster;

        address implementationAddress = _loadAddressOrZero("TOKEN_DISTRIBUTOR_IMPLEMENTATION_ADDRESS");
        TokenDistributor implementation;
        if (implementationAddress == address(0)) {
            implementation = new TokenDistributor();
            implementationAddress = address(implementation);
        } else {
            implementation = TokenDistributor(implementationAddress);
        }

        factory = new TokenDistributorFactory(owner, implementationAddress);

        vm.stopBroadcast();

        console2.log("Deployer (tx.origin):", owner);
        console2.log("TokenDistributorFactory deployed at:", address(factory));
        console2.log("TokenDistributor implementation at:", implementationAddress);
        console2.log("Owner assigned:", owner);
    }

    // No owner loader (owner always broadcaster)

    function _loadAddressOrZero(string memory key) internal view returns (address) {
        try vm.envAddress(key) returns (address provided) {
            return provided;
        } catch {
            return address(0);
        }
    }
}
