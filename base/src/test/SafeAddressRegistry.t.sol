// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";

import {SafeAddressRegistry} from "../SafeAddressRegistry.sol";

contract SafeAddressRegistryTest is Test {
    SafeAddressRegistry private registry;
    address private constant SAFE = address(0xA11CE);
    address private constant OTHER = address(0xBEEF);

    bytes4 private constant OWNABLE_UNAUTHORIZED_SELECTOR = bytes4(keccak256("OwnableUnauthorizedAccount(address)"));
    bytes4 private constant OWNABLE_INVALID_SELECTOR = bytes4(keccak256("OwnableInvalidOwner(address)"));

    function setUp() public {
        registry = new SafeAddressRegistry(SAFE);
    }

    function testConstructorRejectsZeroSafe() public {
        vm.expectRevert(abi.encodeWithSelector(OWNABLE_INVALID_SELECTOR, address(0)));
        new SafeAddressRegistry(address(0));
    }

    function testOwnerIsSafe() public view {
        assertEq(registry.owner(), SAFE);
    }

    function testSetOfferTokenImplementation() public {
        address implementation = address(0x101);

        vm.expectEmit(true, false, false, true, address(registry));
        emit SafeAddressRegistry.OfferTokenImplementationUpdated(implementation);

        vm.prank(SAFE);
        registry.setOfferTokenImplementation(implementation);

        assertEq(registry.offerTokenImplementation(), implementation);
    }

    function testSetOfferTokenImplementationRejectsZero() public {
        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.ZeroAddress.selector);
        registry.setOfferTokenImplementation(address(0));
    }

    function testSetOfferTokenImplementationRejectsSame() public {
        address implementation = address(0x102);
        vm.prank(SAFE);
        registry.setOfferTokenImplementation(implementation);

        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.UnchangedAddress.selector);
        registry.setOfferTokenImplementation(implementation);
    }

    function testSetOfferTokenImplementationOnlyOwner() public {
        vm.prank(OTHER);
        vm.expectRevert(abi.encodeWithSelector(OWNABLE_UNAUTHORIZED_SELECTOR, OTHER));
        registry.setOfferTokenImplementation(address(0x201));
    }

    function testAddOfferTokenProxy() public {
        address proxy = address(0x301);

        vm.expectEmit(true, false, false, true, address(registry));
        emit SafeAddressRegistry.OfferTokenProxyRegistered(proxy);

        vm.prank(SAFE);
        registry.addOfferTokenProxy(proxy);

        address[] memory proxies = registry.getOfferTokenProxies();
        assertEq(proxies.length, 1);
        assertEq(proxies[0], proxy);
    }

    function testAddOfferTokenProxyRejectsDuplicate() public {
        address proxy = address(0x302);
        vm.prank(SAFE);
        registry.addOfferTokenProxy(proxy);

        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.AddressAlreadyTracked.selector);
        registry.addOfferTokenProxy(proxy);
    }

    function testAddOfferTokenProxyRejectsZero() public {
        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.ZeroAddress.selector);
        registry.addOfferTokenProxy(address(0));
    }

    function testRemoveOfferTokenProxy() public {
        address proxyOne = address(0x401);
        address proxyTwo = address(0x402);
        vm.startPrank(SAFE);
        registry.addOfferTokenProxy(proxyOne);
        registry.addOfferTokenProxy(proxyTwo);

        vm.expectEmit(true, false, false, true, address(registry));
        emit SafeAddressRegistry.OfferTokenProxyRemoved(proxyOne);
        registry.removeOfferTokenProxy(proxyOne);
        vm.stopPrank();

        address[] memory proxies = registry.getOfferTokenProxies();
        assertEq(proxies.length, 1);
        assertEq(proxies[0], proxyTwo);
    }

    function testRemoveOfferTokenProxyRejectsUnknown() public {
        address proxy = address(0x501);
        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.AddressNotTracked.selector);
        registry.removeOfferTokenProxy(proxy);
    }

    function testRemoveOfferTokenProxyOnlyOwner() public {
        address proxy = address(0x601);
        vm.prank(SAFE);
        registry.addOfferTokenProxy(proxy);

        vm.prank(OTHER);
        vm.expectRevert(abi.encodeWithSelector(OWNABLE_UNAUTHORIZED_SELECTOR, OTHER));
        registry.removeOfferTokenProxy(proxy);
    }

    function testOfferTokenSaleProxyManagement() public {
        address proxyOne = address(0x611);
        address proxyTwo = address(0x612);
        vm.startPrank(SAFE);
        registry.addOfferTokenSaleProxy(proxyOne);
        registry.addOfferTokenSaleProxy(proxyTwo);
        vm.expectEmit(true, false, false, true, address(registry));
        emit SafeAddressRegistry.OfferTokenSaleProxyRemoved(proxyOne);
        registry.removeOfferTokenSaleProxy(proxyOne);
        vm.stopPrank();

        address[] memory proxies = registry.getOfferTokenSaleProxies();
        assertEq(proxies.length, 1);
        assertEq(proxies[0], proxyTwo);

        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.AddressNotTracked.selector);
        registry.removeOfferTokenSaleProxy(proxyOne);
    }

    function testRemoveOfferTokenSaleProxyRejectsZero() public {
        address proxy = address(0x613);
        vm.prank(SAFE);
        registry.addOfferTokenSaleProxy(proxy);

        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.ZeroAddress.selector);
        registry.removeOfferTokenSaleProxy(address(0));
    }

    function testSetFactoriesAndBeacons() public {
        address offerFactory = address(0x701);
        address offerBeacon = address(0x702);
        address saleFactory = address(0x703);
        address saleBeacon = address(0x704);
        address distributorFactory = address(0x705);
        address distributorBeacon = address(0x706);
        address holdingsFactory = address(0x707);
        address holdingsBeacon = address(0x708);

        vm.startPrank(SAFE);
        registry.setOfferTokenFactory(offerFactory);
        registry.setOfferTokenBeacon(offerBeacon);
        registry.setOfferTokenSaleFactory(saleFactory);
        registry.setOfferTokenSaleBeacon(saleBeacon);
        registry.setTokenDistributorFactory(distributorFactory);
        registry.setTokenDistributorBeacon(distributorBeacon);
        registry.setTokenDistributorByHoldingsFactory(holdingsFactory);
        registry.setTokenDistributorByHoldingsBeacon(holdingsBeacon);
        vm.stopPrank();

        assertEq(registry.offerTokenFactory(), offerFactory);
        assertEq(registry.offerTokenBeacon(), offerBeacon);
        assertEq(registry.offerTokenSaleFactory(), saleFactory);
        assertEq(registry.offerTokenSaleBeacon(), saleBeacon);
        assertEq(registry.tokenDistributorFactory(), distributorFactory);
        assertEq(registry.tokenDistributorBeacon(), distributorBeacon);
        assertEq(registry.tokenDistributorByHoldingsFactory(), holdingsFactory);
        assertEq(registry.tokenDistributorByHoldingsBeacon(), holdingsBeacon);
    }

    function testSetTokenDistributorByHoldingsImplementation() public {
        address implementation = address(0x801);
        vm.prank(SAFE);
        registry.setTokenDistributorByHoldingsImplementation(implementation);
        assertEq(registry.tokenDistributorByHoldingsImplementation(), implementation);
    }

    function testSetTokenDistributorImplementation() public {
        address implementation = address(0x901);
        vm.prank(SAFE);
        registry.setTokenDistributorImplementation(implementation);
        assertEq(registry.tokenDistributorImplementation(), implementation);
    }

    function testSetOfferTokenSaleImplementation() public {
        address implementation = address(0xA01);
        vm.prank(SAFE);
        registry.setOfferTokenSaleImplementation(implementation);
        assertEq(registry.offerTokenSaleImplementation(), implementation);
    }

    function testSetOfferSaleAllowlist() public {
        address allowlist = address(0xB01);
        vm.prank(SAFE);
        registry.setOfferSaleAllowlist(allowlist);
        assertEq(registry.offerSaleAllowlist(), allowlist);

        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.UnchangedAddress.selector);
        registry.setOfferSaleAllowlist(allowlist);
    }

    function testTokenDistributorByHoldingsProxies() public {
        address proxy = address(0xC01);
        vm.prank(SAFE);
        registry.addTokenDistributorByHoldingsProxy(proxy);

        address[] memory proxies = registry.getTokenDistributorByHoldingsProxies();
        assertEq(proxies.length, 1);
        assertEq(proxies[0], proxy);

        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.AddressAlreadyTracked.selector);
        registry.addTokenDistributorByHoldingsProxy(proxy);

        vm.prank(SAFE);
        registry.removeTokenDistributorByHoldingsProxy(proxy);

        proxies = registry.getTokenDistributorByHoldingsProxies();
        assertEq(proxies.length, 0);
    }

    function testTokenDistributorProxies() public {
        address proxyOne = address(0xC10);
        address proxyTwo = address(0xC11);
        vm.startPrank(SAFE);
        registry.addTokenDistributorProxy(proxyOne);
        registry.addTokenDistributorProxy(proxyTwo);
        registry.removeTokenDistributorProxy(proxyTwo);
        vm.stopPrank();

        address[] memory proxies = registry.getTokenDistributorProxies();
        assertEq(proxies.length, 1);
        assertEq(proxies[0], proxyOne);

        vm.prank(SAFE);
        vm.expectRevert(SafeAddressRegistry.ZeroAddress.selector);
        registry.removeTokenDistributorProxy(address(0));
    }
}
