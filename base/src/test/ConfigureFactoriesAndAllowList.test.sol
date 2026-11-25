// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/mocks/token/ERC20Mock.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "../OfferERC20.sol";
import "../OfferTokenFactory.sol";
import "../OfferTokenSale.sol";
import "../OfferTokenSaleFactory.sol";
import "../OfferSaleAllowlist.sol";
import "../TokenDistributor.sol";
import "../TokenDistributorFactory.sol";
import "../TokenDistributorByHoldings.sol";
import "../TokenDistributorByHoldingsFactory.sol";
import {ConfigureFactoriesAndAllowList} from "../../script/ConfigureFactoriesAndAllowList.s.sol";

contract ConfigureFactoriesAndAllowListTest is Test {
    OfferTokenFactory offerTokenFactory;
    OfferTokenSaleFactory offerTokenSaleFactory;
    OfferSaleAllowlist allowlist;
    TokenDistributorFactory tokenDistributorFactory;
    TokenDistributorByHoldingsFactory holdingsFactory;
    ConfigureFactoriesAndAllowList script;

    address owner;
    address controller = address(0xC0);
    address treasury = address(0x5AFE);

    function setUp() public {
        owner = tx.origin;

        offerTokenFactory = new OfferTokenFactory(owner, address(new OfferERC20()));
        offerTokenSaleFactory = new OfferTokenSaleFactory(owner, address(new OfferTokenSale()));
        tokenDistributorFactory = new TokenDistributorFactory(owner, address(new TokenDistributor()));
        holdingsFactory = new TokenDistributorByHoldingsFactory(owner, address(new TokenDistributorByHoldings()));
        allowlist = OfferSaleAllowlist(
            address(
                new ERC1967Proxy(
                    address(new OfferSaleAllowlist()), abi.encodeCall(OfferSaleAllowlist.initialize, (owner))
                )
            )
        );
        script = new ConfigureFactoriesAndAllowList();

        vm.setEnv("OFFER_TOKEN_FACTORY_ADDRESS", vm.toString(address(offerTokenFactory)));
        vm.setEnv("OFFER_TOKEN_SALE_FACTORY_ADDRESS", vm.toString(address(offerTokenSaleFactory)));
        vm.setEnv("OFFER_SALE_ALLOWLIST_ADDRESS", vm.toString(address(allowlist)));
        vm.setEnv("TOKEN_DISTRIBUTOR_FACTORY_ADDRESS", vm.toString(address(tokenDistributorFactory)));
        vm.setEnv("TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS", vm.toString(address(holdingsFactory)));
        vm.setEnv("CONTROLLER_ADDRESS", vm.toString(controller));
    }

    function testGrantsControllerOnEveryContract() public {
        vm.setEnv("CONTROLLER_ENABLED", "true");

        script.run();

        assertTrue(offerTokenFactory.isController(controller));
        assertTrue(offerTokenSaleFactory.isController(controller));
        assertTrue(allowlist.controllers(controller));
        assertTrue(tokenDistributorFactory.isController(controller));
        assertTrue(holdingsFactory.isController(controller));
    }

    function testGrantedControllerCreatesHoldingsDistributorOwnedByTreasury() public {
        vm.setEnv("CONTROLLER_ENABLED", "true");
        script.run();
        address brla = address(new ERC20Mock());
        address offer = address(new ERC20Mock());

        vm.prank(controller);
        address distributor = holdingsFactory.createDistributor(treasury, brla, offer);

        assertEq(TokenDistributorByHoldings(distributor).owner(), treasury);
    }

    function testRevokedControllerCannotCreateHoldingsDistributor() public {
        vm.setEnv("CONTROLLER_ENABLED", "true");
        script.run();
        vm.setEnv("CONTROLLER_ENABLED", "false");
        script.run();

        assertFalse(holdingsFactory.isController(controller));
        vm.prank(controller);
        vm.expectRevert(abi.encodeWithSelector(TokenDistributorByHoldingsFactory.NotController.selector, controller));
        holdingsFactory.createDistributor(treasury, address(0xB1), address(0xB2));
    }
}
