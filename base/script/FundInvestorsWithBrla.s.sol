// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";

/**
 * @title FundInvestorsWithBrla
 * @notice Transfers BRLA from the broadcaster to the configured investor accounts.
 */
contract FundInvestorsWithBrla is Script {
    using SafeERC20 for IERC20;

    function run() external {
        address brlaTokenAddress = vm.envAddress("BRLA_TOKEN_ADDRESS");
        uint256 perInvestorAmount = vm.envUint("BRLA_FUND_PER_INVESTOR_WEI");
        require(perInvestorAmount > 0, "Funding amount must be > 0");

        address[] memory recipients = _loadRecipients();
        IERC20 brla = IERC20(brlaTokenAddress);

        vm.startBroadcast();
        address funder = tx.origin;
        console2.log("BRLA token:", brlaTokenAddress);
        console2.log("Funding from:", funder);
        console2.log("Amount per investor (wei):", perInvestorAmount);

        for (uint256 i = 0; i < recipients.length; i++) {
            address recipient = recipients[i];
            require(recipient != address(0), "Recipient missing");
            brla.safeTransfer(recipient, perInvestorAmount);
            console2.log("Sent", perInvestorAmount, "BRLA wei to", recipient);
        }

        vm.stopBroadcast();
        console2.log("Completed BRLA funding for", recipients.length, "investors");
    }

    function _loadRecipients() private returns (address[] memory recipients) {
        recipients = new address[](3);
        recipients[0] = vm.envAddress("INVESTOR1_ADDRESS");
        recipients[1] = vm.envAddress("INVESTOR2_ADDRESS");
        recipients[2] = vm.envAddress("INVESTOR3_ADDRESS");
    }
}
