// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import "openzeppelin-contracts/contracts/mocks/token/ERC20Mock.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

import "../OfferSaleAllowlist.sol";
import "../OfferTokenSale.sol";
import "../OfferTokenSaleFactory.sol";

interface IOfferTokenSale {
    function owner() external view returns (address);
    function offerToken() external view returns (IERC20);
    function brlaToken() external view returns (IERC20);
    function totalTokensForSale() external view returns (uint256);
}

contract OfferTokenSaleFactoryTest is Test {
    ERC20Mock brla;
    ERC20Mock offer;
    OfferSaleAllowlist allowlist;
    OfferTokenSaleFactory factory;
    OfferTokenSale implementation;
    uint256 hardCap;
    uint256 private constant DEFAULT_COOLING_OFF = 5 days;
    uint256 private constant DEFAULT_MIN_SUCCESS_PERCENT = 75;

    function setUp() public {
        brla = new ERC20Mock();
        offer = new ERC20Mock();
        OfferSaleAllowlist allowlistImplementation = new OfferSaleAllowlist();
        bytes memory allowlistInitData = abi.encodeCall(OfferSaleAllowlist.initialize, (address(this)));
        allowlist = OfferSaleAllowlist(address(new ERC1967Proxy(address(allowlistImplementation), allowlistInitData)));
        implementation = new OfferTokenSale();
        factory = new OfferTokenSaleFactory(address(this), address(implementation));
        // seed supply so totalTokensForSale is non-zero
        offer.mint(address(this), 1000e18);
        brla.mint(address(this), 1000e18);

        hardCap = 1000e18;
    }

    function testHappyPathDeployment() public {
        address initialOwner = address(this);
        address sale = factory.createOfferTokenSale(
            address(offer), address(brla), 1e18, address(allowlist), initialOwner, _defaultSaleConfig()
        );

        IOfferTokenSale saleContract = IOfferTokenSale(sale);
        assertEq(saleContract.owner(), initialOwner);
        assertEq(address(saleContract.offerToken()), address(offer));
        assertEq(address(saleContract.brlaToken()), address(brla));
        assertEq(saleContract.totalTokensForSale(), offer.totalSupply());
    }

    function testRevertsOnInvalidInputs() public {
        // zero offer token
        vm.expectRevert("Offer token required");
        factory.createOfferTokenSale(
            address(0), address(brla), 1e18, address(allowlist), address(this), _defaultSaleConfig()
        );
        // zero BRLA token
        vm.expectRevert("BRLA token required");
        factory.createOfferTokenSale(
            address(offer), address(0), 1e18, address(allowlist), address(this), _defaultSaleConfig()
        );
        // zero allowlist
        vm.expectRevert("Allowlist required");
        factory.createOfferTokenSale(
            address(offer), address(brla), 1e18, address(0), address(this), _defaultSaleConfig()
        );
        // zero owner
        vm.expectRevert("Owner required");
        factory.createOfferTokenSale(
            address(offer), address(brla), 1e18, address(allowlist), address(0), _defaultSaleConfig()
        );
        // zero price
        vm.expectRevert("Price must be positive");
        factory.createOfferTokenSale(
            address(offer), address(brla), 0, address(allowlist), address(this), _defaultSaleConfig()
        );
    }

    function _defaultSaleConfig() internal view returns (OfferTokenSale.SaleConfig memory) {
        return OfferTokenSale.SaleConfig({
            hardCap: hardCap, coolingOffPeriod: DEFAULT_COOLING_OFF, minSuccessPercent: DEFAULT_MIN_SUCCESS_PERCENT
        });
    }

    function testUpgradeImplementation() public {
        OfferTokenSale newImplementation = new OfferTokenSale();
        vm.expectEmit(true, false, false, true, address(factory));
        emit OfferTokenSaleFactory.BeaconUpgraded(address(newImplementation));
        factory.upgradeImplementation(address(newImplementation));

        assertEq(factory.beacon().implementation(), address(newImplementation));
    }

    function testUpgradeImplementationOnlyOwner() public {
        OfferTokenSale newImplementation = new OfferTokenSale();
        vm.prank(address(0xBEEF));
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, address(0xBEEF)));
        factory.upgradeImplementation(address(newImplementation));
    }

    function testOnlyControllerCanCreateSale() public {
        address attacker = address(0xBAD);

        vm.prank(attacker);
        vm.expectRevert(abi.encodeWithSelector(OfferTokenSaleFactory.NotController.selector, attacker));
        factory.createOfferTokenSale(
            address(offer), address(brla), 1e18, address(allowlist), address(this), _defaultSaleConfig()
        );

        factory.setController(attacker, true);

        vm.prank(attacker);
        address sale = factory.createOfferTokenSale(
            address(offer), address(brla), 1e18, address(allowlist), address(this), _defaultSaleConfig()
        );
        assertTrue(sale != address(0));
        assertTrue(factory.isController(attacker));
    }

    function testCreatesSaleWithFiveMinuteCoolingOff() public {
        OfferTokenSale.SaleConfig memory config = _defaultSaleConfig();
        config.coolingOffPeriod = 5 minutes;

        address sale = factory.createOfferTokenSale(
            address(offer), address(brla), 1e18, address(allowlist), address(this), config
        );

        assertEq(OfferTokenSale(sale).coolingOffPeriod(), 5 minutes);
    }

    function testRevertsOnCoolingOffBelowOneMinute() public {
        OfferTokenSale.SaleConfig memory config = _defaultSaleConfig();
        config.coolingOffPeriod = 59 seconds;

        vm.expectRevert("Cooling-off too short");
        factory.createOfferTokenSale(address(offer), address(brla), 1e18, address(allowlist), address(this), config);
    }

    function testZeroCoolingOffAppliesTheFiveDayDefault() public {
        OfferTokenSale.SaleConfig memory config = _defaultSaleConfig();
        config.coolingOffPeriod = 0;

        address sale = factory.createOfferTokenSale(
            address(offer), address(brla), 1e18, address(allowlist), address(this), config
        );

        assertEq(OfferTokenSale(sale).coolingOffPeriod(), 5 days);
    }
}
