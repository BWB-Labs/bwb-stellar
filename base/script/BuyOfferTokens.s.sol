// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {OfferTokenSale} from "../src/OfferTokenSale.sol";

/**
 * @title BuyOfferTokens
 * @notice Foundry script that approves BRLA and buys offer tokens from an
 *         existing OfferTokenSale using a provided private key.
 */
contract BuyOfferTokens is Script {
    struct PurchaseParams {
        address saleAddress;
        uint256 brlaAmount;
    }

    /**
     * @notice Approves BRLA and purchases offer tokens from a sale contract.
     * @return tokensBought Amount of offer tokens received.
     */
    function run() external returns (uint256 tokensBought) {
        PurchaseParams memory params = _loadParams();
        OfferTokenSale sale = OfferTokenSale(params.saleAddress);
        IERC20 brla = IERC20(address(sale.brlaToken()));

        console2.log("Preparing purchase via OfferTokenSale:", params.saleAddress);
        console2.log("BRLA token address:", address(brla));
        console2.log("Offer token address:", address(sale.offerToken()));
        console2.log("Requested BRLA amount:", params.brlaAmount);

        uint256 previewTokens = sale.previewTokens(params.brlaAmount);
        console2.log("Preview offer tokens (before slippage checks):", previewTokens);

        vm.startBroadcast();

        address buyer = tx.origin;
        if (buyer == address(0)) {
            vm.stopBroadcast();
            revert("Unable to infer buyer from broadcast; provide --sender");
        }
        console2.log("Buyer address:", buyer);
        uint256 buyerBalance = brla.balanceOf(buyer);
        console2.log("Buyer BRLA balance:", buyerBalance);
        if (buyerBalance < params.brlaAmount) {
            vm.stopBroadcast();
            revert("Insufficient BRLA balance for purchase");
        }

        brla.approve(params.saleAddress, params.brlaAmount);

        try sale.buyWithBrla(params.brlaAmount) returns (uint256 purchased) {
            tokensBought = purchased;
        } catch Error(string memory reason) {
            vm.stopBroadcast();
            console2.log("OfferTokenSale::buyWithBrla reverted with:", reason);
            revert(string.concat("OfferTokenSale buy failed: ", reason));
        } catch (bytes memory lowLevelData) {
            vm.stopBroadcast();
            string memory decoded = _decodeLowLevelRevert(lowLevelData);
            console2.log("OfferTokenSale::buyWithBrla reverted (low-level):", decoded);
            revert(decoded);
        }

        vm.stopBroadcast();

        console2.log("OfferTokenSale:", params.saleAddress);
        console2.log("BRLA spent:", params.brlaAmount);
        console2.log("Offer tokens received:", tokensBought);
        console2.log("Offer token address:", address(sale.offerToken()));
    }

    function _loadParams() internal view returns (PurchaseParams memory params) {
        params.saleAddress = vm.envAddress("OFFER_TOKEN_SALE_ADDRESS");
        params.brlaAmount = vm.envUint("BRLA_AMOUNT");
        require(params.brlaAmount > 0, "BRLA amount required");
    }

    function _decodeLowLevelRevert(bytes memory revertData) internal pure returns (string memory) {
        if (revertData.length >= 68) {
            bytes4 selector;
            assembly {
                selector := mload(add(revertData, 32))
            }
            if (selector == 0x08c379a0) {
                bytes memory reasonBytes = new bytes(revertData.length - 4);
                for (uint256 i = 4; i < revertData.length; i++) {
                    reasonBytes[i - 4] = revertData[i];
                }
                string memory decodedReason = abi.decode(reasonBytes, (string));
                return string(abi.encodePacked("OfferTokenSale buy failed: ", decodedReason));
            }
        }

        if (revertData.length == 0) {
            return "OfferTokenSale buy failed with empty revert data";
        }

        return "OfferTokenSale buy failed with unknown revert data";
    }
}
