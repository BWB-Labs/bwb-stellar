// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferERC20} from "../src/OfferERC20.sol";

/**
 * @title ApproveOfferTokenInventory
 * @notice Grants an OfferTokenSale contract permission to pull the full OfferERC20 supply.
 */
contract ApproveOfferTokenInventory is Script {
    function run() external {
        address offerTokenAddress = vm.envAddress("OFFER_TOKEN_ADDRESS");
        address saleAddress = vm.envAddress("OFFER_TOKEN_SALE_ADDRESS");

        OfferERC20 offerToken = OfferERC20(offerTokenAddress);

        uint256 totalSupply = offerToken.totalSupply();
        require(totalSupply > 0, "Offer token supply missing");

        vm.startBroadcast();
        offerToken.approve(saleAddress, totalSupply);
        vm.stopBroadcast();

        console2.log("Approved OfferTokenSale to spend", totalSupply, "tokens");
        console2.log("Sale address:", saleAddress);
    }
}
