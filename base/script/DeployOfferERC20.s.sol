// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferERC20} from "../src/OfferERC20.sol";
import {OfferTokenFactory} from "../src/OfferTokenFactory.sol";

/**
 * @title DeployOfferERC20
 * @notice Foundry script that uses an existing OfferTokenFactory to deploy
 *         a new OfferERC20 instance using parameters provided via env vars.
 */
contract DeployOfferERC20 is Script {
    struct DeploymentParams {
        address factory;
        string name;
        string symbol;
        address owner;
        uint256 initialSupply;
        string offerURI;
    }

    /**
     * @notice Deploys a new OfferERC20 token via an existing factory
     * @return token The deployed OfferERC20 instance
     */
    function run() external returns (OfferERC20 token) {
        DeploymentParams memory params = _loadParams();
        OfferTokenFactory factory = OfferTokenFactory(params.factory);

        vm.startBroadcast();

        address resolvedOwner = params.owner;
        require(resolvedOwner != address(0), "Owner address required");
        address transferAdminOverride = _loadOwnerOrZero("TRANSFER_ADMIN_ADDRESS");
        address transferAdmin = transferAdminOverride != address(0) ? transferAdminOverride : resolvedOwner;

        address deployedToken = factory.createOfferToken(
            params.name, params.symbol, resolvedOwner, params.initialSupply, params.offerURI, transferAdmin
        );

        vm.stopBroadcast();

        token = OfferERC20(deployedToken);
        console2.log("OfferERC20 deployed at:", address(token));
        console2.log("Factory used:", address(factory));
        console2.log("Owner receiving supply:", resolvedOwner);
        console2.log("Offer URI:", params.offerURI);
    }

    function _loadParams() internal view returns (DeploymentParams memory params) {
        params.factory = vm.envAddress("FACTORY_ADDRESS");
        params.name = vm.envString("TOKEN_NAME");
        params.symbol = vm.envString("TOKEN_SYMBOL");

        address ownerOverride = _loadOwnerOrZero("TOKEN_OWNER");
        if (ownerOverride != address(0)) {
            params.owner = ownerOverride;
        } else {
            params.owner = vm.envAddress("DEPLOYER_ADDRESS");
        }

        params.initialSupply = vm.envUint("INITIAL_SUPPLY");
        params.offerURI = vm.envString("OFFER_URI");
    }

    function _loadOwnerOrZero(string memory key) internal view returns (address owner) {
        try vm.envAddress(key) returns (address providedOwner) {
            owner = providedOwner;
        } catch {
            owner = address(0);
        }
    }
}
