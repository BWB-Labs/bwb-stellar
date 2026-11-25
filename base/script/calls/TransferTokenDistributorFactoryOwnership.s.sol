// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";

import {TokenDistributorFactory} from "../../src/TokenDistributorFactory.sol";

/// @notice Foundry script that transfers ownership of an existing TokenDistributorFactory.
contract TransferTokenDistributorFactoryOwnership is Script {
    struct Params {
        address factory;
        address newOwner;
    }

    function run() external {
        Params memory p = _loadParams();

        console2.log("Transferring TokenDistributorFactory ownership");
        console2.log("Factory:", p.factory);
        console2.log("New owner:", p.newOwner);

        vm.startBroadcast();

        TokenDistributorFactory factory = TokenDistributorFactory(p.factory);
        console2.log("Current owner:", factory.owner());

        factory.transferOwnership(p.newOwner);

        vm.stopBroadcast();

        console2.log("Ownership transfer submitted");
    }

    function _loadParams() internal view returns (Params memory p) {
        p.factory = vm.envAddress("TOKEN_DISTRIBUTOR_FACTORY_ADDRESS");
        p.newOwner = vm.envAddress("NEW_TOKEN_DISTRIBUTOR_FACTORY_OWNER");
        require(p.factory != address(0), "Factory address required");
        require(p.newOwner != address(0), "New owner required");
    }
}
