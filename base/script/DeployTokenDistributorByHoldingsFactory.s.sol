// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {TokenDistributorByHoldingsFactory} from "../src/TokenDistributorByHoldingsFactory.sol";
import {TokenDistributorByHoldings} from "../src/TokenDistributorByHoldings.sol";

/**
 * @title DeployTokenDistributorByHoldingsFactory
 * @notice Foundry script that deploys the TokenDistributorByHoldingsFactory contract.
 */
contract DeployTokenDistributorByHoldingsFactory is Script {
    /**
     * @notice Deploys the factory and logs its address.
     * @return factory The deployed TokenDistributorByHoldingsFactory instance.
     */
    function run() external returns (TokenDistributorByHoldingsFactory factory) {
        vm.startBroadcast();

        (, address broadcaster,) = vm.readCallers();
        address owner = _loadOwnerOrDefault("TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_OWNER", broadcaster);

        address implementationAddress = _loadAddressOrZero("TOKEN_DISTRIBUTOR_BY_HOLDINGS_IMPLEMENTATION_ADDRESS");
        TokenDistributorByHoldings implementation;
        if (implementationAddress == address(0)) {
            implementation = new TokenDistributorByHoldings();
            implementationAddress = address(implementation);
        } else {
            implementation = TokenDistributorByHoldings(implementationAddress);
        }

        factory = new TokenDistributorByHoldingsFactory(owner, implementationAddress);

        vm.stopBroadcast();

        console2.log("TokenDistributorByHoldingsFactory deployed at:", address(factory));
        console2.log("TokenDistributorByHoldings implementation at:", implementationAddress);
        console2.log("Owner assigned:", owner);
    }

    function _loadOwnerOrDefault(string memory key, address fallbackOwner) internal view returns (address) {
        address supplied = _loadAddressOrZero(key);
        if (supplied != address(0)) {
            return supplied;
        }
        return fallbackOwner;
    }

    function _loadAddressOrZero(string memory key) internal view returns (address) {
        try vm.envAddress(key) returns (address provided) {
            return provided;
        } catch {
            return address(0);
        }
    }
}
