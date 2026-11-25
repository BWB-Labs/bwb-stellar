// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "../OfferTokenSale.sol";
import "../OfferSaleAllowlist.sol";
import "./mocks/MockUSDC.sol";

contract OfferTokenSaleTest is Test {
    OfferTokenSale sale;
    OfferSaleAllowlist allowlist;
    MockUSDC offerToken;
    MockUSDC brlaToken;

    address owner = address(this);
    address buyer = address(0xD);
    address other = address(0xBEEF);
    address[] buyers;
    uint256[] maxes;
    bool[] enabled;

    uint256 private defaultHardCap;
    uint256 private constant DEFAULT_COOLING_OFF = 5 days;
    uint256 private constant DEFAULT_MIN_SUCCESS_PERCENT = 75;

    function setUp() public {
        delete buyers;
        delete maxes;
        delete enabled;

        offerToken = new MockUSDC(10000e18);
        brlaToken = new MockUSDC(5000e18);
        OfferSaleAllowlist allowlistImplementation = new OfferSaleAllowlist();
        bytes memory allowlistInitData = abi.encodeCall(OfferSaleAllowlist.initialize, (owner));
        allowlist = OfferSaleAllowlist(address(new ERC1967Proxy(address(allowlistImplementation), allowlistInitData)));

        defaultHardCap = 500e18;

        sale = deploySaleProxy(
            address(offerToken),
            address(brlaToken),
            5e17,
            address(allowlist),
            owner,
            defaultHardCap,
            DEFAULT_COOLING_OFF,
            DEFAULT_MIN_SUCCESS_PERCENT
        );

        buyers.push(buyer);
        maxes.push(100e18);
        enabled.push(true);

        bool prevController = allowlist.controllers(address(this));
        if (!prevController) {
            allowlist.setController(address(this), true);
        }

        allowlist.setAllocations(buyers, maxes, enabled);

        if (!prevController) {
            allowlist.setController(address(this), false);
        }
        allowlist.setController(address(sale), true);

        offerToken.transfer(address(sale), 100e18);
        brlaToken.transfer(buyer, 50e18);
    }

    function deploySaleProxy(
        address offerToken_,
        address brlaToken_,
        uint256 brlaPerToken_,
        address allowlist_,
        address owner_,
        uint256 hardCap_,
        uint256 coolingOffPeriod_,
        uint256 minSuccessPercent_
    ) internal returns (OfferTokenSale) {
        OfferTokenSale implementation = new OfferTokenSale();
        OfferTokenSale.SaleConfig memory config = OfferTokenSale.SaleConfig({
            hardCap: hardCap_, coolingOffPeriod: coolingOffPeriod_, minSuccessPercent: minSuccessPercent_
        });
        bytes memory initData = abi.encodeCall(
            OfferTokenSale.initialize, (offerToken_, brlaToken_, brlaPerToken_, allowlist_, owner_, config)
        );
        return OfferTokenSale(address(new ERC1967Proxy(address(implementation), initData)));
    }

    function deploySaleProxyExternal(
        address offerToken_,
        address brlaToken_,
        uint256 brlaPerToken_,
        address allowlist_,
        address owner_,
        uint256 hardCap_,
        uint256 coolingOffPeriod_,
        uint256 minSuccessPercent_
    ) external returns (OfferTokenSale) {
        return deploySaleProxy(
            offerToken_, brlaToken_, brlaPerToken_, allowlist_, owner_, hardCap_, coolingOffPeriod_, minSuccessPercent_
        );
    }

    function testConstructorRevertOfferTokenZero() public {
        vm.expectRevert("Offer token required");
        this.deploySaleProxyExternal(
            address(0),
            address(brlaToken),
            1e18,
            address(allowlist),
            owner,
            defaultHardCap,
            DEFAULT_COOLING_OFF,
            DEFAULT_MIN_SUCCESS_PERCENT
        );
    }

    function testConstructorRevertBrlaTokenZero() public {
        vm.expectRevert("BRLA token required");
        this.deploySaleProxyExternal(
            address(offerToken),
            address(0),
            1e18,
            address(allowlist),
            owner,
            defaultHardCap,
            DEFAULT_COOLING_OFF,
            DEFAULT_MIN_SUCCESS_PERCENT
        );
    }

    function testConstructorRevertPriceZero() public {
        vm.expectRevert("Price must be positive");
        this.deploySaleProxyExternal(
            address(offerToken),
            address(brlaToken),
            0,
            address(allowlist),
            owner,
            defaultHardCap,
            DEFAULT_COOLING_OFF,
            DEFAULT_MIN_SUCCESS_PERCENT
        );
    }

    function testConstructorRevertAllowlistZero() public {
        vm.expectRevert("Allowlist required");
        this.deploySaleProxyExternal(
            address(offerToken),
            address(brlaToken),
            1e18,
            address(0),
            owner,
            defaultHardCap,
            DEFAULT_COOLING_OFF,
            DEFAULT_MIN_SUCCESS_PERCENT
        );
    }

    function testConstructorRevertOwnerZero() public {
        vm.expectRevert("Owner required");
        this.deploySaleProxyExternal(
            address(offerToken),
            address(brlaToken),
            1e18,
            address(allowlist),
            address(0),
            defaultHardCap,
            DEFAULT_COOLING_OFF,
            DEFAULT_MIN_SUCCESS_PERCENT
        );
    }

    function testPreviewTokens() public {
        uint256 amount = sale.previewTokens(1e18);
        assertEq(amount, 2);
    }
}
