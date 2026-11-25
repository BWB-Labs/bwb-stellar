// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";

import {OfferSaleAllowlist} from "../../src/OfferSaleAllowlist.sol";

/// @notice Transfers ownership of the OfferSaleAllowlist proxy to a new owner (Safe).
contract TransferOfferSaleAllowlistOwnership is Script {
    struct Params {
        address allowlistProxy;
        address newOwner;
    }

    function run() external {
        Params memory p = _loadParams();

        console2.log("Transferring OfferSaleAllowlist ownership");
        console2.log("Allowlist proxy:", p.allowlistProxy);
        console2.log("New owner:", p.newOwner);

        OfferSaleAllowlist allowlist = OfferSaleAllowlist(p.allowlistProxy);

        address currentOwner = allowlist.owner();
        console2.log("Current owner:", currentOwner);

        require(currentOwner != address(0), "Owner is zero; wrong address?");
        require(currentOwner != p.newOwner, "New owner already assigned");

        vm.startBroadcast();

        allowlist.transferOwnership(p.newOwner);

        vm.stopBroadcast();

        address ownerAfter = allowlist.owner();
        console2.log("Owner after:", ownerAfter);

        require(ownerAfter == p.newOwner, "Ownership transfer failed");

        console2.log("Ownership transfer submitted");
    }

    function _loadParams() internal view returns (Params memory p) {
        p.allowlistProxy = vm.envAddress("OFFER_SALE_ALLOWLIST_PROXY_ADDRESS");
        p.newOwner = vm.envAddress("NEW_OFFER_SALE_ALLOWLIST_OWNER");

        require(p.allowlistProxy != address(0), "Allowlist proxy address required");
        require(p.newOwner != address(0), "New owner required");

        _requireHasCode(p.allowlistProxy);
    }

    function _requireHasCode(address target) private view {
        uint256 codeSize;
        assembly {
            codeSize := extcodesize(target)
        }
        require(codeSize > 0, "Allowlist proxy has no code");
    }
}
