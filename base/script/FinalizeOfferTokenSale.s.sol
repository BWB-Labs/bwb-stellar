// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferTokenSale} from "../src/OfferTokenSale.sol";

/**
 * @title FinalizeOfferTokenSale
 * @notice Broadcasts finalizeSale for a deployed OfferTokenSale once it is sold out.
 */
contract FinalizeOfferTokenSale is Script {
    function run() external {
        address saleAddress = vm.envAddress("OFFER_TOKEN_SALE_ADDRESS");
        OfferTokenSale sale = OfferTokenSale(saleAddress);

        uint256 sold = sale.soldTokens();
        uint256 total = sale.totalTokensForSale();

        console2.log("Attempting to finalize OfferTokenSale:", saleAddress);
        console2.log("Sold tokens:", sold);
        console2.log("Total tokens:", total);

        vm.startBroadcast();
        sale.finalizeSale();
        vm.stopBroadcast();

        console2.log("OfferTokenSale finalized:", saleAddress);
    }
}
