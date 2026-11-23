// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";

import "../TokenDistributorFactory.sol";
import "../TokenDistributor.sol";
import "./mocks/TokenDistributorV2.sol";

contract TokenDistributorFactoryTest is Test {
    TokenDistributorFactory factory;
    TokenDistributor implementation;
    TokenDistributorV2 implementationV2;

    address factoryOwner = address(0x1);
    address user = address(0x2);

    string constant NAME = "Test Distributor";
    string constant DESCRIPTION = "ipfs://description";

    function setUp() public {
        implementation = new TokenDistributor();
        factory = new TokenDistributorFactory(factoryOwner, address(implementation));
    }

    function _createDistributor(address owner_, string memory name_, string memory description_)
        internal
        returns (address)
    {
        vm.prank(factoryOwner);
        return factory.createTokenDistributor(owner_, name_, description_);
    }

    function testDeploymentSetsBeaconImplementation() public {
        address beaconAddress = address(factory.beacon());
        address beaconImplementation = factory.beacon().implementation();

        assertTrue(beaconAddress != address(0), "Beacon address is zero");
        assertEq(beaconImplementation, address(implementation), "Implementation mismatch");
        assertTrue(beaconImplementation != address(factory), "Implementation cannot be factory");
    }

    function testCreateTokenDistributor() public {
        address proxy = _createDistributor(factoryOwner, NAME, DESCRIPTION);

        assertTrue(proxy != address(0), "Proxy address is zero");
        assertTrue(proxy != address(factory), "Proxy cannot be factory");
        assertTrue(proxy != address(implementation), "Proxy cannot be implementation");

        TokenDistributor distributor = TokenDistributor(proxy);
        assertEq(distributor.owner(), factoryOwner);
        assertEq(distributor.name(), NAME);
        assertEq(distributor.descriptionURI(), DESCRIPTION);
    }

    function testCreateMultipleProxies() public {
        address proxy1 = _createDistributor(factoryOwner, "Dist1", "ipfs://desc1");
        address proxy2 = _createDistributor(user, "Dist2", "ipfs://desc2");

        assertTrue(proxy1 != proxy2, "Proxies should have different addresses");

        TokenDistributor dist1 = TokenDistributor(proxy1);
        TokenDistributor dist2 = TokenDistributor(proxy2);

        assertEq(dist1.owner(), factoryOwner);
        assertEq(dist2.owner(), user);
        assertEq(dist1.name(), "Dist1");
        assertEq(dist2.name(), "Dist2");
        assertEq(dist1.descriptionURI(), "ipfs://desc1");
        assertEq(dist2.descriptionURI(), "ipfs://desc2");
    }

    bytes4 private constant OWNABLE_UNAUTHORIZED_SELECTOR = bytes4(keccak256("OwnableUnauthorizedAccount(address)"));

    function testOnlyOwnerCanUpgradeImplementation() public {
        implementationV2 = new TokenDistributorV2();

        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OWNABLE_UNAUTHORIZED_SELECTOR, user));
        factory.upgradeImplementation(address(implementationV2));
    }

    function testOwnerCanUpgradeImplementation() public {
        implementationV2 = new TokenDistributorV2();

        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        address beaconImplementation = factory.beacon().implementation();
        assertEq(beaconImplementation, address(implementationV2), "Beacon did not upgrade");
    }

    function testUpgradeChangesLogicForExistingProxy() public {
        address proxy = _createDistributor(factoryOwner, NAME, DESCRIPTION);

        implementationV2 = new TokenDistributorV2();

        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        string memory version = TokenDistributorV2(proxy).versionString();
        assertEq(version, "2.0.0", "Existing proxy did not pick up new logic");
    }

    function testUpgradeKeepsStateForExistingProxy() public {
        address proxy = _createDistributor(factoryOwner, NAME, DESCRIPTION);

        TokenDistributor beforeUpgrade = TokenDistributor(proxy);
        assertEq(beforeUpgrade.owner(), factoryOwner);
        assertEq(beforeUpgrade.name(), NAME);
        assertEq(beforeUpgrade.descriptionURI(), DESCRIPTION);

        implementationV2 = new TokenDistributorV2();
        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        TokenDistributor afterUpgrade = TokenDistributor(proxy);
        assertEq(afterUpgrade.owner(), factoryOwner, "Owner changed after upgrade");
        assertEq(afterUpgrade.name(), NAME, "Name changed after upgrade");
        assertEq(afterUpgrade.descriptionURI(), DESCRIPTION, "Description changed after upgrade");
    }

    function testNewProxiesUseUpgradedImplementation() public {
        implementationV2 = new TokenDistributorV2();

        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        address proxy = _createDistributor(factoryOwner, NAME, DESCRIPTION);
        string memory version = TokenDistributorV2(proxy).versionString();
        assertEq(version, "2.0.0", "New proxy not using upgraded implementation");
    }

    function testUpgradedFunctionWorksAndRespectsState() public {
        address proxy = _createDistributor(factoryOwner, NAME, DESCRIPTION);

        implementationV2 = new TokenDistributorV2();
        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        vm.prank(factoryOwner);
        TokenDistributorV2(proxy).setDescriptionURI("ipfs://new-description");

        assertEq(
            TokenDistributor(proxy).descriptionURI(),
            "ipfs://new-description",
            "Upgraded logic did not modify stored state correctly"
        );
    }

    function testNonOwnerCannotUseNewOnlyOwnerFunctionAfterUpgrade() public {
        address proxy = _createDistributor(factoryOwner, NAME, DESCRIPTION);

        implementationV2 = new TokenDistributorV2();
        vm.prank(factoryOwner);
        factory.upgradeImplementation(address(implementationV2));

        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        TokenDistributorV2(proxy).setDescriptionURI("ipfs://hacked");
    }

    function testOnlyControllerCanCreateDistributor() public {
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(TokenDistributorFactory.NotController.selector, user));
        factory.createTokenDistributor(user, NAME, DESCRIPTION);

        vm.prank(factoryOwner);
        factory.setController(user, true);

        vm.prank(user);
        address proxy = factory.createTokenDistributor(user, NAME, DESCRIPTION);
        assertTrue(proxy != address(0));
        assertTrue(factory.isController(user));
    }
}
