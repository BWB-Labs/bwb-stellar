// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferTokenSale} from "../src/OfferTokenSale.sol";

/**
 * @title ReleaseOfferTokens
 * @notice Foundry script that releases finalized offer tokens to a list of investor addresses.
 */
contract ReleaseOfferTokens is Script {
    /**
     * @notice Calls OfferTokenSale.releaseTokens with the recipients provided via environment variable.
     */
    function run() external {
        OfferTokenSale sale = OfferTokenSale(vm.envAddress("OFFER_TOKEN_SALE_ADDRESS"));
        address[] memory recipients = _loadRecipients();

        console2.log("Releasing tokens from sale", address(sale));
        console2.log("Recipient count", recipients.length);

        vm.startBroadcast();
        sale.releaseTokens(recipients);
        vm.stopBroadcast();

        console2.log("Token release complete");
    }

    function _loadRecipients() private view returns (address[] memory recipients) {
        string memory rawRecipients = vm.envString("RELEASE_RECIPIENTS_JSON");
        require(bytes(rawRecipients).length > 0, "RELEASE_RECIPIENTS_JSON required");
        recipients = abi.decode(vm.parseJson(rawRecipients), (address[]));
        require(recipients.length > 0, "Recipients list empty");
    }
}
