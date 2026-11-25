// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferTokenSale} from "../src/OfferTokenSale.sol";
import {OfferTokenSaleFactory} from "../src/OfferTokenSaleFactory.sol";

/**
 * @title DeployOfferTokenSaleFactory
 * @notice Foundry script that deploys the OfferTokenSaleFactory contract using a
 *         named Forge account/keystore.
 */
contract DeployOfferTokenSaleFactory is Script {
    /**
     * @notice Deploy the OfferTokenSaleFactory contract
     * @return factory The deployed OfferTokenSaleFactory instance
     */
    function run() external returns (OfferTokenSaleFactory factory) {
        vm.startBroadcast();

        (, address broadcaster,) = vm.readCallers();
        address owner = _loadOwnerOrDefault("OFFER_TOKEN_SALE_FACTORY_OWNER", broadcaster);

        console2.log("Owner assigned:", owner);

        address implementationAddress = _loadAddressOrZero("OFFER_TOKEN_SALE_IMPLEMENTATION_ADDRESS");
        if (implementationAddress == address(0)) {
            implementationAddress = address(new OfferTokenSale());
        }

        factory = new OfferTokenSaleFactory(owner, implementationAddress);

        vm.stopBroadcast();

        console2.log("OfferTokenSaleFactory deployed at:", address(factory));
        console2.log("OfferTokenSale implementation at:", implementationAddress);
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
