// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferERC20} from "../src/OfferERC20.sol";
import {OfferTokenSale} from "../src/OfferTokenSale.sol";

/**
 * @title SeedOfferTokenSale
 * @notice Approves and seeds a deployed OfferTokenSale with the full OfferERC20 supply.
 */
contract SeedOfferTokenSale is Script {
    function run() external {
        address offerTokenAddress = vm.envAddress("OFFER_TOKEN_ADDRESS");
        address saleAddress = vm.envAddress("OFFER_TOKEN_SALE_ADDRESS");
        address inventoryProvider = _loadInventoryProvider();

        OfferERC20 offerToken = OfferERC20(offerTokenAddress);
        OfferTokenSale sale = OfferTokenSale(saleAddress);

        uint256 totalSupply = offerToken.totalSupply();
        require(totalSupply > 0, "Offer token supply missing");

        vm.startBroadcast();

        if (inventoryProvider != address(0) && sale.inventoryProvider() != inventoryProvider) {
            sale.updateInventoryProvider(inventoryProvider);
        }

        sale.seedInventory();

        vm.stopBroadcast();

        console2.log("OfferTokenSale seeded with", totalSupply, "tokens");
        console2.log("Sale address:", saleAddress);
        if (inventoryProvider != address(0)) {
            console2.log("Inventory provider:", inventoryProvider);
        }
    }

    function _loadInventoryProvider() private view returns (address provider) {
        try vm.envAddress("INVENTORY_PROVIDER_ADDRESS") returns (address supplied) {
            provider = supplied;
        } catch {
            provider = address(0);
        }
    }
}
