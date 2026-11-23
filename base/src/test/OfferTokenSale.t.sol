// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import {ERC1967Proxy} from "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {OfferTokenSale} from "../OfferTokenSale.sol";
import {OfferERC20} from "../OfferERC20.sol";
import {OfferSaleAllowlist} from "../OfferSaleAllowlist.sol";
import {BRLA} from "./mocks/BRLA.sol";

contract OfferTokenSaleTest is Test {
    OfferERC20 private offerToken;
    BRLA private brla;
    OfferSaleAllowlist private allowlist;
    OfferTokenSale private sale;
    OfferTokenSale private saleImplementation;

    address private constant SALE_OWNER = address(0xA11CE);
    address private constant INVENTORY_PROVIDER = address(0xBEEF);
    address private constant BUYER_ONE = address(0xB01);
    address private constant BUYER_TWO = address(0xB02);
    address private constant RANDOM_ADDRESS = address(0xB0B);
    address private constant TREASURY = address(0xFEE1);
    address private constant TRANSFER_ADMIN = address(0xAD100);

    uint256 private constant TOKEN_PRICE = 1 ether;
    uint256 private constant TOTAL_OFFER_TOKEN_SUPPLY = 120;
    uint256 private constant DEFAULT_MIN_SUCCESS_PERCENT = 75;
    uint256 private constant DEFAULT_COOLING_OFF = 5 days;
    uint256 private constant TOTAL_BRLA_SUPPY = 1_000_0000 ether;

    struct SaleConfig {
        //hardCap is the maximum BRLA amount possible to receive in the contract
        uint256 hardCap;
        uint256 coolingOffPeriod;
        uint256 minSuccessPercent;
    }

    SaleConfig private defaultSaleConfig;

    function setUp() public {
        OfferERC20 ercImplementation = new OfferERC20();
        bytes memory initData = abi.encodeCall(
            OfferERC20.initialize,
            (
                "Tower One",
                "TOWER",
                INVENTORY_PROVIDER,
                TOTAL_OFFER_TOKEN_SUPPLY,
                "ipfs://tower.json",
                TRANSFER_ADMIN,
                new address[](0)
            )
        );
        offerToken = OfferERC20(address(new ERC1967Proxy(address(ercImplementation), initData)));
        brla = new BRLA(TOTAL_BRLA_SUPPY);
        OfferSaleAllowlist allowlistImplementation = new OfferSaleAllowlist();
        bytes memory allowlistInitData = abi.encodeCall(OfferSaleAllowlist.initialize, (address(this)));
        allowlist = OfferSaleAllowlist(address(new ERC1967Proxy(address(allowlistImplementation), allowlistInitData)));
        saleImplementation = new OfferTokenSale();

        defaultSaleConfig = SaleConfig({
            hardCap: 120 ether, coolingOffPeriod: DEFAULT_COOLING_OFF, minSuccessPercent: DEFAULT_MIN_SUCCESS_PERCENT
        });

        _resetSaleWithConfig(defaultSaleConfig);

        vm.label(SALE_OWNER, "saleOwner");
        vm.label(INVENTORY_PROVIDER, "inventoryProvider");
        vm.label(BUYER_ONE, "buyerOne");
        vm.label(BUYER_TWO, "buyerTwo");
        vm.label(TREASURY, "treasury");
    }

    function testSaleStateTransitions() public {
        _assertSaleState(OfferTokenSale.SaleState.Preparing);
        _seedInventoryFromProvider();
        _assertSaleState(OfferTokenSale.SaleState.Active);
        _configureAllocation(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY * TOKEN_PRICE);
        _buyTokens(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY);
        vm.prank(SALE_OWNER);
        sale.finalizeSale();
        _assertSaleState(OfferTokenSale.SaleState.Successful);
    }

    function testOwnerCanSeedInventoryUsingExternalProvider() public {
        _seedInventoryFromProvider();
        assertTrue(sale.inventorySeeded());
        assertEq(offerToken.balanceOf(address(sale)), TOTAL_OFFER_TOKEN_SUPPLY);
        assertEq(offerToken.balanceOf(INVENTORY_PROVIDER), 0);
    }

    function testCannotUpdateInventoryProviderAfterSeeding() public {
        _seedInventoryFromProvider();
        vm.expectRevert(bytes("Inventory seeded"));
        vm.prank(SALE_OWNER);
        sale.updateInventoryProvider(RANDOM_ADDRESS);
    }

    function testNonOwnerCannotUpdateInventoryProvider() public {
        vm.expectRevert(abi.encodeWithSignature("OwnableUnauthorizedAccount(address)", RANDOM_ADDRESS));
        vm.prank(RANDOM_ADDRESS);
        sale.updateInventoryProvider(RANDOM_ADDRESS);
    }

    function testUpdateInventoryProviderRejectsZeroAddress() public {
        vm.expectRevert(bytes("Provider required"));
        vm.prank(SALE_OWNER);
        sale.updateInventoryProvider(address(0));
    }

    function testSeedInventoryCannotBeCalledTwice() public {
        _seedInventoryFromProvider();
        vm.expectRevert(bytes("Sale not preparing"));
        vm.prank(SALE_OWNER);
        sale.seedInventory();
    }

    function testPreviewTokens() public {
        uint256 amount = sale.previewTokens(5 ether);
        assertEq(amount, 5);
    }

    function testBuyWithBrlaReservesTokensAndRecordsPayment() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 10 ether);
        uint256 brlaPaid = _buyTokens(BUYER_ONE, 10);

        assertEq(sale.tokensPurchased(BUYER_ONE), 10);
        assertEq(sale.brlaSpent(BUYER_ONE), brlaPaid);
        assertEq(sale.soldTokens(), 10);
        assertEq(sale.totalBrlaRaised(), brlaPaid);
    }

    function testAllowlistTracksBrlaSpent() public {
        _seedInventoryFromProvider();
        uint256 brlaAmount = 15 ether;
        _configureAllocation(BUYER_ONE, brlaAmount);
        uint256 brlaPaid = _buyTokens(BUYER_ONE, 5);

        assertEq(allowlist.remainingAllocation(BUYER_ONE), brlaAmount - brlaPaid);
    }

    function testBuyWithBrlaRevertsWhenInventoryNotSeeded() public {
        _configureAllocation(BUYER_ONE, 1 ether);
        uint256 brlaAmount = TOKEN_PRICE;
        brla.mint(BUYER_ONE, brlaAmount);
        vm.prank(BUYER_ONE);
        brla.approve(address(sale), brlaAmount);

        vm.expectRevert(bytes("Inventory not seeded"));
        vm.prank(BUYER_ONE);
        sale.buyWithBrla(brlaAmount);
    }

    function testBuyWithBrlaRevertsWhenSaleFinalized() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY * TOKEN_PRICE);
        _buyTokens(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY);
        vm.prank(SALE_OWNER);
        sale.finalizeSale();

        _configureAllocation(BUYER_TWO, 1 ether);
        uint256 brlaAmount = TOKEN_PRICE;
        brla.mint(BUYER_TWO, brlaAmount);
        vm.prank(BUYER_TWO);
        brla.approve(address(sale), brlaAmount);

        vm.expectRevert(bytes("Sale not active"));
        vm.prank(BUYER_TWO);
        sale.buyWithBrla(brlaAmount);
    }

    function testBuyWithBrlaRevertsWhenRefundsActive() public {
        _seedInventoryFromProvider();
        vm.prank(SALE_OWNER);
        sale.enableRefunds();

        _configureAllocation(BUYER_ONE, 1 ether);
        uint256 brlaAmount = TOKEN_PRICE;
        brla.mint(BUYER_ONE, brlaAmount);
        vm.prank(BUYER_ONE);
        brla.approve(address(sale), brlaAmount);

        vm.expectRevert(bytes("Sale not active"));
        vm.prank(BUYER_ONE);
        sale.buyWithBrla(brlaAmount);
    }

    function testBuyWithBrlaRequiresExactPrice() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 1 ether);
        uint256 brlaAmount = TOKEN_PRICE - 1;
        brla.mint(BUYER_ONE, brlaAmount);
        vm.prank(BUYER_ONE);
        brla.approve(address(sale), brlaAmount);

        vm.expectRevert(bytes("BRLA must match whole tokens"));
        vm.prank(BUYER_ONE);
        sale.buyWithBrla(brlaAmount);
    }

    function testBuyWithBrlaRevertsWhenHardCapExceeded() public {
        SaleConfig memory config = defaultSaleConfig;
        config.hardCap = 90 ether;
        _resetSaleWithConfig(config);

        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 100 ether);
        _configureAllocation(BUYER_TWO, 100 ether);

        _buyTokens(BUYER_ONE, 60);

        uint256 brlaAmount = 40 * TOKEN_PRICE;
        brla.mint(BUYER_TWO, brlaAmount);
        vm.prank(BUYER_TWO);
        brla.approve(address(sale), brlaAmount);

        vm.expectRevert(bytes("Hard cap exceeded"));
        vm.prank(BUYER_TWO);
        sale.buyWithBrla(brlaAmount);
    }

    function testFinalizeSaleRevertsWhenSuccessCriteriaNotMet() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 10 ether);
        _buyTokens(BUYER_ONE, 10);

        vm.expectRevert(bytes("Success criteria unmet"));
        vm.prank(SALE_OWNER);
        sale.finalizeSale();
    }

    function testFinalizeSaleAndClaimTokens() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY * TOKEN_PRICE);
        _buyTokens(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY);

        vm.prank(SALE_OWNER);
        sale.finalizeSale();
        assertTrue(sale.isFinalized());
        _assertSaleState(OfferTokenSale.SaleState.Successful);

        address[] memory recipients = new address[](1);
        recipients[0] = BUYER_ONE;

        sale.releaseTokens(recipients);

        assertEq(offerToken.balanceOf(BUYER_ONE), TOTAL_OFFER_TOKEN_SUPPLY);
        assertEq(sale.tokensPurchased(BUYER_ONE), 0);
        assertEq(sale.brlaSpent(BUYER_ONE), 0);
    }

    function testFinalizeSaleSucceedsAtMinPercent() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY * TOKEN_PRICE);
        _buyTokens(BUYER_ONE, 90);

        vm.prank(SALE_OWNER);
        sale.finalizeSale();

        assertTrue(sale.isFinalized());
        _assertSaleState(OfferTokenSale.SaleState.Successful);
    }

    function testMarkSaleFailedWhenCriteriaUnmet() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 30 ether);
        _buyTokens(BUYER_ONE, 30);

        vm.prank(SALE_OWNER);
        sale.markSaleFailed();

        assertTrue(sale.refundsEnabled());
        _assertSaleState(OfferTokenSale.SaleState.Failed);

        address[] memory recipients = new address[](1);
        recipients[0] = BUYER_ONE;
        sale.processRefunds(recipients);

        assertEq(brla.balanceOf(BUYER_ONE), 30 * TOKEN_PRICE);
    }

    function testReleaseTokensDistributesForMultipleRecipients() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 70 ether);
        _buyTokens(BUYER_ONE, 70);
        _configureAllocation(BUYER_TWO, (TOTAL_OFFER_TOKEN_SUPPLY - 70) * TOKEN_PRICE);
        _buyTokens(BUYER_TWO, TOTAL_OFFER_TOKEN_SUPPLY - 70);

        vm.prank(SALE_OWNER);
        sale.finalizeSale();

        address[] memory recipients = new address[](2);
        recipients[0] = BUYER_ONE;
        recipients[1] = BUYER_TWO;

        sale.releaseTokens(recipients);

        assertEq(offerToken.balanceOf(BUYER_ONE), 70);
        assertEq(offerToken.balanceOf(BUYER_TWO), TOTAL_OFFER_TOKEN_SUPPLY - 70);
        assertEq(sale.tokensPurchased(BUYER_ONE), 0);
        assertEq(sale.tokensPurchased(BUYER_TWO), 0);
    }

    function testEnableRefundsAndClaimRefund() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 20 ether);
        uint256 brlaPaid = _buyTokens(BUYER_ONE, 20);

        vm.prank(SALE_OWNER);
        sale.enableRefunds();
        _assertSaleState(OfferTokenSale.SaleState.Failed);

        address[] memory recipients = new address[](1);
        recipients[0] = BUYER_ONE;

        sale.processRefunds(recipients);

        uint256 refunded = brla.balanceOf(BUYER_ONE);
        assertEq(refunded, brlaPaid);
        assertEq(brla.balanceOf(BUYER_ONE), brlaPaid);
        assertEq(sale.tokensPurchased(BUYER_ONE), 0);
        assertEq(sale.soldTokens(), 0);
        assertEq(sale.totalBrlaRaised(), 0);
    }

    function testCoolingOffRefundWithinWindow() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 10 ether);
        uint256 brlaPaid = _buyTokens(BUYER_ONE, 10);

        vm.prank(BUYER_ONE);
        uint256 refunded = sale.requestCoolingOffRefund();

        assertEq(refunded, brlaPaid);
        assertEq(sale.tokensPurchased(BUYER_ONE), 0);
        assertEq(sale.soldTokens(), 0);
        assertEq(brla.balanceOf(BUYER_ONE), brlaPaid);
        assertEq(sale.totalBrlaRaised(), 0);
    }

    function testCoolingOffRefundAfterFinalizationWithinWindow() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY * TOKEN_PRICE);
        uint256 brlaPaid = _buyTokens(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY);

        vm.prank(SALE_OWNER);
        sale.finalizeSale();
        _assertSaleState(OfferTokenSale.SaleState.Successful);

        vm.prank(BUYER_ONE);
        uint256 refunded = sale.requestCoolingOffRefund();

        assertEq(refunded, brlaPaid);
        assertEq(sale.tokensPurchased(BUYER_ONE), 0);
        assertEq(sale.soldTokens(), 0);
        assertEq(brla.balanceOf(BUYER_ONE), brlaPaid);
        assertEq(sale.totalBrlaRaised(), 0);
    }

    function testCoolingOffRefundAfterFinalizationExpiredWindowReverts() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY * TOKEN_PRICE);
        _buyTokens(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY);

        vm.prank(SALE_OWNER);
        sale.finalizeSale();
        _assertSaleState(OfferTokenSale.SaleState.Successful);

        vm.warp(block.timestamp + sale.coolingOffPeriod() + 1);

        vm.expectRevert(bytes("Cooling-off expired"));
        vm.prank(BUYER_ONE);
        sale.requestCoolingOffRefund();
    }

    function testCoolingOffRefundRevertsAfterWindow() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 5 ether);
        _buyTokens(BUYER_ONE, 5);

        vm.warp(block.timestamp + sale.coolingOffPeriod() + 1);

        vm.expectRevert(bytes("Cooling-off expired"));
        vm.prank(BUYER_ONE);
        sale.requestCoolingOffRefund();
    }

    function testProcessRefundsRefundsMultipleBuyers() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 30 ether);
        uint256 paidBuyerOne = _buyTokens(BUYER_ONE, 30);
        _configureAllocation(BUYER_TWO, 40 ether);
        uint256 paidBuyerTwo = _buyTokens(BUYER_TWO, 40);

        vm.prank(SALE_OWNER);
        sale.enableRefunds();

        address[] memory recipients = new address[](2);
        recipients[0] = BUYER_ONE;
        recipients[1] = BUYER_TWO;

        sale.processRefunds(recipients);

        assertEq(brla.balanceOf(BUYER_ONE), paidBuyerOne);
        assertEq(brla.balanceOf(BUYER_TWO), paidBuyerTwo);
        assertEq(sale.soldTokens(), 0);
        assertEq(sale.totalBrlaRaised(), 0);
    }

    function testWithdrawBrlaTransfersFundsAfterFinalization() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY * TOKEN_PRICE);
        _buyTokens(BUYER_ONE, TOTAL_OFFER_TOKEN_SUPPLY);

        vm.prank(SALE_OWNER);
        sale.finalizeSale();

        uint256 withdrawAmount = 25 ether;
        vm.prank(SALE_OWNER);
        sale.withdrawBrla(TREASURY, withdrawAmount);

        assertEq(brla.balanceOf(TREASURY), withdrawAmount);
    }

    function testWithdrawOfferTokensAfterRefundsEnabled() public {
        _seedInventoryFromProvider();
        _configureAllocation(BUYER_ONE, 30 ether);
        _buyTokens(BUYER_ONE, 30);

        vm.prank(SALE_OWNER);
        sale.enableRefunds();

        vm.prank(SALE_OWNER);
        sale.withdrawOfferTokens(TREASURY, 40);

        assertEq(offerToken.balanceOf(TREASURY), 40);
    }

    function _deploySale(SaleConfig memory config) private returns (OfferTokenSale deployedSale) {
        OfferTokenSale.SaleConfig memory onChainConfig = OfferTokenSale.SaleConfig({
            hardCap: config.hardCap,
            coolingOffPeriod: config.coolingOffPeriod,
            minSuccessPercent: config.minSuccessPercent
        });

        bytes memory saleInit = abi.encodeCall(
            OfferTokenSale.initialize,
            (address(offerToken), address(brla), TOKEN_PRICE, address(allowlist), SALE_OWNER, onChainConfig)
        );
        deployedSale = OfferTokenSale(address(new ERC1967Proxy(address(saleImplementation), saleInit)));
    }

    function _resetSaleWithConfig(SaleConfig memory config) private {
        sale = _deploySale(config);
        allowlist.setController(address(sale), true);
        vm.prank(INVENTORY_PROVIDER);
        offerToken.setTransferWhitelist(address(sale), true);
    }

    function _seedInventoryFromProvider() private {
        uint256 supply = offerToken.totalSupply();

        vm.prank(SALE_OWNER);
        sale.updateInventoryProvider(INVENTORY_PROVIDER);
        vm.prank(INVENTORY_PROVIDER);
        offerToken.approve(address(sale), supply);
        vm.prank(SALE_OWNER);
        sale.seedInventory();
    }

    function _configureAllocation(address buyer, uint256 brlaAmount) private {
        bool prevControllerState = allowlist.controllers(address(this));
        if (!prevControllerState) {
            allowlist.setController(address(this), true);
        }

        address[] memory buyers = new address[](1);
        buyers[0] = buyer;

        uint256[] memory allocations = new uint256[](1);
        allocations[0] = brlaAmount;

        bool[] memory enabled = new bool[](1);
        enabled[0] = true;

        allowlist.setAllocations(buyers, allocations, enabled);

        if (!prevControllerState) {
            allowlist.setController(address(this), false);
        }
    }

    // receives the buyer address and the amount of offerTokens it wants to buy
    function _buyTokens(address buyer, uint256 tokensToPurchase) private returns (uint256 brlaAmount) {
        brlaAmount = tokensToPurchase * TOKEN_PRICE;
        brla.mint(buyer, brlaAmount);
        vm.prank(buyer);
        brla.approve(address(sale), brlaAmount);
        vm.prank(buyer);
        sale.buyWithBrla(brlaAmount);
    }

    function _assertSaleState(OfferTokenSale.SaleState expected) private {
        assertEq(uint8(sale.saleState()), uint8(expected));
    }
}
