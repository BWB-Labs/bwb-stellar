// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/mocks/token/ERC20Mock.sol";

import "../TokenDistributorByHoldingsFactory.sol";
import "../TokenDistributorByHoldings.sol";
import "./mocks/TokenDistributorByHoldingsV2.sol";

contract TokenDistributorByHoldingsFactoryTest is Test {
    TokenDistributorByHoldingsFactory factory;
    TokenDistributorByHoldings implementation;
    TokenDistributorByHoldingsV2 implementationV2;

    ERC20Mock brla;
    ERC20Mock offer;

    address factoryOwner = address(0x1);
    address user = address(0x2);

    bytes4 private constant OWNABLE_UNAUTHORIZED_SELECTOR = bytes4(keccak256("OwnableUnauthorizedAccount(address)"));

    function setUp() public {
        implementation = new TokenDistributorByHoldings();
        factory = new TokenDistributorByHoldingsFactory(factoryOwner, address(implementation));
        brla = new ERC20Mock();
        offer = new ERC20Mock();
    }

    function createDistributor(address owner_) internal returns (TokenDistributorByHoldings) {
        vm.prank(factoryOwner);
        address proxy = factory.createDistributor(owner_, address(brla), address(offer));
        return TokenDistributorByHoldings(proxy);
    }

    function testDeploymentSetsBeaconImplementation() public {
        address beaconAddress = address(factory.beacon());
        address beaconImplementation = factory.beacon().implementation();

        assertTrue(beaconAddress != address(0), "Beacon address is zero");
        assertEq(beaconImplementation, address(implementation), "Implementation mismatch");
        assertTrue(beaconImplementation != address(factory), "Implementation cannot be factory");
    }

    function testCreateDistributorSetsOwnershipAndParams() public {
        address owner = address(0xBEEF);
        TokenDistributorByHoldings distributor = createDistributor(owner);

        assertEq(distributor.owner(), owner);
        assertEq(address(distributor.paymentToken()), address(brla));
        assertEq(address(distributor.offerToken()), address(offer));
    }

    function testCreateDistributorEmitsEvent() public {
        address owner = address(0xC0FFEE);
        vm.expectEmit(false, true, true, true, address(factory));
        emit TokenDistributorByHoldingsFactory.TokenDistributorByHoldingsCreated(
            address(0), owner, address(brla), address(offer)
        );

        createDistributor(owner);
    }

    function testOnlyOwnerCanUpgradeImplementation() public {
        implementationV2 = new TokenDistributorByHoldingsV2();

        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OWNABLE_UNAUTHORIZED_SELECTOR, user));
        factory.upgradeImplementation(address(implementationV2));
    }

    function testOwnerCanUpgradeImplementation() public {
        implementationV2 = new TokenDistributorByHoldingsV2();

        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        address beaconImplementation = factory.beacon().implementation();
        assertEq(beaconImplementation, address(implementationV2), "Beacon did not upgrade");
    }

    function testUpgradeChangesLogicForExistingProxy() public {
        TokenDistributorByHoldings distributor = createDistributor(factoryOwner);

        implementationV2 = new TokenDistributorByHoldingsV2();
        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        string memory version = TokenDistributorByHoldingsV2(address(distributor)).versionString();
        assertEq(version, "2.0.0", "Existing proxy did not pick up new logic");
    }

    function testUpgradeKeepsStateForExistingProxy() public {
        TokenDistributorByHoldings distributor = createDistributor(factoryOwner);

        address beforeOwner = distributor.owner();
        assertEq(beforeOwner, factoryOwner);
        assertEq(address(distributor.paymentToken()), address(brla));
        assertEq(address(distributor.offerToken()), address(offer));

        implementationV2 = new TokenDistributorByHoldingsV2();
        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        TokenDistributorByHoldings afterUpgrade = TokenDistributorByHoldings(address(distributor));
        assertEq(afterUpgrade.owner(), factoryOwner, "Owner changed after upgrade");
        assertEq(address(afterUpgrade.paymentToken()), address(brla), "Payment token changed after upgrade");
        assertEq(address(afterUpgrade.offerToken()), address(offer), "Offer token changed after upgrade");
    }

    function testNewProxiesUseUpgradedImplementation() public {
        implementationV2 = new TokenDistributorByHoldingsV2();

        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        TokenDistributorByHoldings distributor = createDistributor(factoryOwner);
        string memory version = TokenDistributorByHoldingsV2(address(distributor)).versionString();
        assertEq(version, "2.0.0", "New proxy not using upgraded implementation");
    }

    function testUpgradedFunctionWorksAndRespectsState() public {
        TokenDistributorByHoldings distributor = createDistributor(factoryOwner);

        implementationV2 = new TokenDistributorByHoldingsV2();
        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        vm.prank(factoryOwner);
        TokenDistributorByHoldingsV2(address(distributor)).bumpCumulativePerToken(1);

        assertEq(distributor.cumulativePerToken(), 1, "Upgraded logic failed to adjust accumulator");
    }

    function testNonOwnerCannotUseNewOnlyOwnerFunctionAfterUpgrade() public {
        TokenDistributorByHoldings distributor = createDistributor(factoryOwner);

        implementationV2 = new TokenDistributorByHoldingsV2();
        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OWNABLE_UNAUTHORIZED_SELECTOR, user));
        TokenDistributorByHoldingsV2(address(distributor)).bumpCumulativePerToken(1);
    }

    function testOnlyControllerCanCreateDistributor() public {
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(TokenDistributorByHoldingsFactory.NotController.selector, user));
        factory.createDistributor(user, address(brla), address(offer));

        vm.prank(factoryOwner);
        factory.setController(user, true);

        vm.prank(user);
        address proxy = factory.createDistributor(user, address(brla), address(offer));
        assertTrue(proxy != address(0));
        assertTrue(factory.isController(user));
    }
}
