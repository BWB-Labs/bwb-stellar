// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {OfferTokenSale} from "../src/OfferTokenSale.sol";

/**
 * @title WithdrawOfferSaleBrla
 * @notice Foundry script that withdraws BRLA proceeds from an OfferTokenSale to an advisor wallet.
 */
contract WithdrawOfferSaleBrla is Script {
    /**
     * @notice Executes the withdrawal transaction from the sale contract to the configured advisor address.
     */
    function run() external {
        OfferTokenSale sale = OfferTokenSale(vm.envAddress("OFFER_TOKEN_SALE_ADDRESS"));
        address advisor = vm.envAddress("ADVISOR_ADDRESS");
        uint256 amount = _resolveWithdrawAmount(sale);

        console2.log("Withdrawing BRLA from sale", address(sale));
        console2.log("Advisor recipient", advisor);
        console2.log("Amount", amount);

        vm.startBroadcast();
        sale.withdrawBrla(advisor, amount);
        vm.stopBroadcast();

        console2.log("BRLA withdrawal completed");
    }

    function _resolveWithdrawAmount(OfferTokenSale sale) private view returns (uint256 amount) {
        try vm.envUint("BRLA_WITHDRAW_AMOUNT") returns (uint256 overrideAmount) {
            amount = overrideAmount;
        } catch {
            IERC20 brla = IERC20(sale.brlaToken());
            amount = brla.balanceOf(address(sale));
        }
        require(amount > 0, "No BRLA available to withdraw");
    }
}
