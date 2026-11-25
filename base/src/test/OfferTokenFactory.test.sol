// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import "../OfferTokenFactory.sol";
import "../OfferERC20.sol";

contract OfferTokenFactoryTest is Test {
    OfferTokenFactory factory;
    OfferERC20 implementation;
    address owner = address(0x1);
    address user = address(0x2);
    address transferAdmin = address(0x5);

    function setUp() public {
        implementation = new OfferERC20();
        factory = new OfferTokenFactory(address(this), address(implementation));
    }

    function testCreateOfferToken() public {
        string memory name = "Test Offering";
        string memory symbol = "TOFER";
        uint256 supply = 1000 ether;
        string memory uri = "ipfs://metadata";

        address tokenAddress = factory.createOfferToken(name, symbol, owner, supply, uri, transferAdmin);

        OfferERC20 token = OfferERC20(tokenAddress);
        assertEq(token.name(), name);
        assertEq(token.symbol(), symbol);
        assertEq(token.balanceOf(owner), supply);
        assertEq(token.offerURI(), uri);
        assertEq(token.owner(), owner);
    }

    function testEmitOfferTokenDeployedEvent() public {
        string memory name = "Test Offering";
        string memory symbol = "TOFER";
        uint256 supply = 1000 ether;
        string memory uri = "ipfs://metadata";

        address tokenAddress = factory.createOfferToken(name, symbol, owner, supply, uri, transferAdmin);
        assertTrue(tokenAddress != address(0), "Token not created");
    }

    function testUpgradeImplementation() public {
        OfferERC20 replacement = new OfferERC20();
        vm.expectEmit(true, false, false, true, address(factory));
        emit OfferTokenFactory.BeaconUpgraded(address(replacement));
        factory.upgradeImplementation(address(replacement));

        assertEq(factory.beacon().implementation(), address(replacement));
    }

    function testUpgradeImplementationOnlyOwner() public {
        OfferERC20 replacement = new OfferERC20();
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        factory.upgradeImplementation(address(replacement));
    }

    function testCannotCreateWithZeroOwner() public {
        string memory name = "Test Offering";
        string memory symbol = "TOFER";
        uint256 supply = 1000 ether;
        string memory uri = "ipfs://metadata";

        vm.expectRevert("Owner cannot be zero address");
        factory.createOfferToken(name, symbol, address(0), supply, uri, transferAdmin);
    }

    function testCreateMultipleTokens() public {
        string memory uri1 = "ipfs://metadata1";
        string memory uri2 = "ipfs://metadata2";

        address token1 = factory.createOfferToken("Token1", "TK1", owner, 500 ether, uri1, transferAdmin);
        address token2 = factory.createOfferToken("Token2", "TK2", owner, 750 ether, uri2, transferAdmin);

        assertTrue(token1 != token2, "Tokens should have different addresses");

        OfferERC20 offer1 = OfferERC20(token1);
        OfferERC20 offer2 = OfferERC20(token2);

        assertEq(offer1.offerURI(), uri1);
        assertEq(offer2.offerURI(), uri2);
    }

    function testOnlyControllerCanCreateTokens() public {
        address nonController = address(0xBEEF);

        vm.prank(nonController);
        vm.expectRevert(abi.encodeWithSelector(OfferTokenFactory.NotController.selector, nonController));
        factory.createOfferToken("Token3", "TK3", owner, 100 ether, "ipfs://uri", transferAdmin);

        factory.setController(nonController, true);

        vm.prank(nonController);
        address token = factory.createOfferToken("Token4", "TK4", owner, 100 ether, "ipfs://uri2", transferAdmin);
        assertTrue(token != address(0));
        assertTrue(factory.isController(nonController));
    }
}
