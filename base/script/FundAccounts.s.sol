// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";

/// @title FundAccounts
/// @notice Sends small native-token allocations from the broadcaster (deployer)
///         to advisor and investor accounts so they can pay testnet gas.
contract FundAccounts is Script {
    struct Recipient {
        string label;
        address account;
        uint256 amountWei;
    }

    uint256 internal constant RECIPIENT_AMOUNT_WEI = 0.01 ether;
    bool internal constant FUND_ONLY_EMPTY = true; // toggle to fund even if they already hold gas

    /// @notice Broadcast transfers from the configured funder to advisor/investors.
    function run() external {
        Recipient[] memory recipients = _loadRecipients();

        vm.startBroadcast();
        address funder = tx.origin;
        if (funder == address(0)) {
            vm.stopBroadcast();
            revert("Broadcast sender unavailable; pass --account/--sender");
        }

        if (funder.balance == 0) {
            vm.stopBroadcast();
            revert("Funder has zero balance");
        }

        console2.log("Funder:", funder);
        console2.log("Initial funder balance:", funder.balance);

        uint256 totalSent;
        for (uint256 i = 0; i < recipients.length; i++) {
            Recipient memory recipient = recipients[i];
            if (recipient.account == address(0)) {
                vm.stopBroadcast();
                revert(string.concat("Recipient ", recipient.label, " is zero"));
            }

            if (FUND_ONLY_EMPTY && recipient.account.balance > 0) {
                console2.log("Skipping", recipient.label, "-> already funded");
                continue;
            }

            if (recipient.amountWei == 0) {
                console2.log("Skipping", recipient.label, "-> amount zero");
                continue;
            }

            if (funder.balance < recipient.amountWei) {
                vm.stopBroadcast();
                revert("Insufficient balance for funding");
            }

            (bool success,) = payable(recipient.account).call{value: recipient.amountWei}("");
            if (!success) {
                vm.stopBroadcast();
                revert(string.concat("Transfer to ", recipient.label, " failed"));
            }

            totalSent += recipient.amountWei;
            console2.log("Sent", recipient.amountWei, "wei to", recipient.account);
        }

        vm.stopBroadcast();

        console2.log("Total distributed:", totalSent);
        console2.log("Remaining funder balance:", funder.balance);
    }

    function _loadRecipients() internal returns (Recipient[] memory recipients) {
        recipients = new Recipient[](4);
        recipients[0] = _buildRecipient("advisor", "ADVISOR_ADDRESS");
        recipients[1] = _buildRecipient("investor1", "INVESTOR1_ADDRESS");
        recipients[2] = _buildRecipient("investor2", "INVESTOR2_ADDRESS");
        recipients[3] = _buildRecipient("investor3", "INVESTOR3_ADDRESS");
    }

    function _buildRecipient(string memory label, string memory envKey) internal returns (Recipient memory) {
        return Recipient({label: label, account: _requireEnvAddress(envKey), amountWei: RECIPIENT_AMOUNT_WEI});
    }

    function _requireEnvAddress(string memory key) internal returns (address account) {
        try vm.envAddress(key) returns (address provided) {
            account = provided;
        } catch {
            revert(string.concat("Missing environment variable ", key));
        }

        if (account == address(0)) {
            revert(string.concat("Environment variable ", key, " is zero"));
        }

        return account;
    }
}
