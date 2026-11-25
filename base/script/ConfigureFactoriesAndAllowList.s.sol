// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferTokenFactory} from "../src/OfferTokenFactory.sol";
import {OfferTokenSaleFactory} from "../src/OfferTokenSaleFactory.sol";
import {OfferSaleAllowlist} from "../src/OfferSaleAllowlist.sol";
import {TokenDistributorFactory} from "../src/TokenDistributorFactory.sol";
import {TokenDistributorByHoldingsFactory} from "../src/TokenDistributorByHoldingsFactory.sol";

contract ConfigureFactoriesAndAllowList is Script {
    struct Params {
        address offerTokenFactory;
        address offerTokenSaleFactory;
        address offerSaleAllowlist;
        address tokenDistributorFactory;
        address tokenDistributorByHoldingsFactory;
        address controller;
        bool enabled;
    }

    function run() external {
        Params memory p = _loadParams();

        console2.log("Configuring controller for all factories & allowlist:");
        console2.log("Controller:", p.controller);
        console2.log("Enabled:", p.enabled);

        vm.startBroadcast();

        OfferTokenFactory(p.offerTokenFactory).setController(p.controller, p.enabled);
        OfferTokenSaleFactory(p.offerTokenSaleFactory).setController(p.controller, p.enabled);
        OfferSaleAllowlist(p.offerSaleAllowlist).setController(p.controller, p.enabled);
        TokenDistributorFactory(p.tokenDistributorFactory).setController(p.controller, p.enabled);
        TokenDistributorByHoldingsFactory(p.tokenDistributorByHoldingsFactory).setController(p.controller, p.enabled);

        vm.stopBroadcast();

        console2.log("All controllers configured successfully.");
    }

    function _loadParams() internal view returns (Params memory p) {
        p.offerTokenFactory = vm.envAddress("OFFER_TOKEN_FACTORY_ADDRESS");
        p.offerTokenSaleFactory = vm.envAddress("OFFER_TOKEN_SALE_FACTORY_ADDRESS");
        p.offerSaleAllowlist = vm.envAddress("OFFER_SALE_ALLOWLIST_ADDRESS");
        p.tokenDistributorFactory = vm.envAddress("TOKEN_DISTRIBUTOR_FACTORY_ADDRESS");
        p.tokenDistributorByHoldingsFactory = vm.envAddress("TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS");
        p.controller = vm.envAddress("CONTROLLER_ADDRESS");
        p.enabled = vm.envBool("CONTROLLER_ENABLED");
    }
}
