// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import {ERC1967Proxy} from "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";

import "../OfferERC20.sol";

contract OfferERC20Harness is OfferERC20 {
    function mintTo(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function burnFrom(address from, uint256 amount) external {
        _burn(from, amount);
    }
}

/// @title Test suite for OfferERC20 token
/// @notice Covers deployment, ownership, metadata URI updates, and ERC20 functions
contract OfferERC20Test is Test {
    OfferERC20 offerToken;
    address owner = address(0x1);
    address user = address(0x2);
    address otherUser = address(0x3);
    address distributor = address(0x4);
    address transferAdmin = address(0x99);
    string constant TOKEN_NAME = "Test Offering";
    string constant TOKEN_SYMBOL = "TOFER";
    string constant INITIAL_URI = "ipfs://initial-metadata";
    string constant UPDATED_URI = "ipfs://updated-metadata";
    uint256 initialSupply = 1_000_000 ether;

    function setUp() public {
        offerToken = _deployOfferToken(new address[](0));
    }

    function testHasCorrectMetadata() public {
        assertEq(offerToken.name(), TOKEN_NAME, "Token name incorrect");
        assertEq(offerToken.symbol(), TOKEN_SYMBOL, "Token symbol incorrect");
        assertEq(offerToken.decimals(), 0, "Decimals must be 0");
        assertEq(offerToken.offerURI(), INITIAL_URI, "Initial URI incorrect");
    }

    function testInitialSupplyMintedToOwner() public {
        assertEq(offerToken.totalSupply(), initialSupply, "Total supply mismatch");
        assertEq(offerToken.balanceOf(owner), initialSupply, "Owner did not receive initial supply");
        assertEq(offerToken.balanceOf(address(this)), 0, "Deployer should have no tokens");
        assertTrue(offerToken.isTransferWhitelisted(owner));
    }

    function testInitialWhitelistedAccountsConfigured() public {
        address[] memory whitelist = new address[](2);
        whitelist[0] = owner;
        whitelist[1] = distributor;

        OfferERC20 alternateToken = _deployOfferToken(whitelist);

        assertTrue(alternateToken.isTransferWhitelisted(owner));
        assertTrue(alternateToken.isTransferWhitelisted(distributor));
    }

    function testInitialWhitelistRejectsZeroAddress() public {
        address[] memory whitelist = new address[](1);
        whitelist[0] = address(0);

        OfferERC20 implementation = new OfferERC20();
        bytes memory initData = abi.encodeCall(
            OfferERC20.initialize,
            (TOKEN_NAME, TOKEN_SYMBOL, owner, initialSupply, INITIAL_URI, transferAdmin, whitelist)
        );

        vm.expectRevert(bytes("Whitelisted account required"));
        new ERC1967Proxy(address(implementation), initData);
    }

    function testUpdateOfferURIPerformedByOwner() public {
        vm.prank(owner);
        offerToken.updateOfferURI(UPDATED_URI);

        assertEq(offerToken.offerURI(), UPDATED_URI, "URI not updated");
    }

    function testUpdateOfferURIEmitsEvent() public {
        vm.prank(owner);
        vm.expectEmit(false, false, false, true, address(offerToken));
        emit OfferERC20.OfferURIUpdated(UPDATED_URI);
        offerToken.updateOfferURI(UPDATED_URI);
    }

    function testVersionString() public {
        assertEq(offerToken.version(), "1.0.0", "Version mismatch");
    }

    function testNonOwnerCannotUpdateOfferURI() public {
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        offerToken.updateOfferURI(UPDATED_URI);

        assertEq(offerToken.offerURI(), INITIAL_URI, "URI should not change");
    }

    function testTransferTokens() public {
        uint256 transferAmount = 500 ether;

        vm.prank(owner);
        offerToken.transfer(user, transferAmount);

        assertEq(offerToken.balanceOf(user), transferAmount, "User did not receive tokens");
        assertEq(offerToken.balanceOf(owner), initialSupply - transferAmount, "Owner balance incorrect");
    }

    function testTransferBetweenUnapprovedUsersReverts() public {
        uint256 transferAmount = 100 ether;
        vm.prank(owner);
        offerToken.transfer(user, transferAmount);

        vm.expectRevert(bytes("Transfer restricted"));
        vm.prank(user);
        offerToken.transfer(otherUser, 10 ether);
    }

    function testUserCanReturnTokensToWhitelistedAddress() public {
        vm.prank(owner);
        offerToken.transfer(user, 50 ether);

        assertTrue(offerToken.isTransferWhitelisted(owner));

        vm.prank(user);
        offerToken.transfer(owner, 50 ether);

        assertEq(offerToken.balanceOf(owner), initialSupply);
    }

    function testWhitelistedContractCanDistributeTokens() public {
        vm.prank(owner);
        offerToken.setTransferWhitelist(distributor, true);

        vm.prank(owner);
        offerToken.transfer(distributor, 200 ether);

        vm.prank(distributor);
        offerToken.transfer(user, 75 ether);

        assertEq(offerToken.balanceOf(user), 75 ether);
    }

    function testNonOwnerCannotUpdateWhitelist() public {
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        vm.prank(user);
        offerToken.setTransferWhitelist(distributor, true);
    }

    function testUpdateURITransfersOwnership() public {
        address newOwner = address(0x3);

        vm.prank(owner);
        offerToken.transferOwnership(newOwner);
        assertEq(offerToken.owner(), newOwner, "Ownership transfer failed");

        vm.prank(newOwner);
        offerToken.updateOfferURI("ipfs://new-metadata");
        assertEq(offerToken.offerURI(), "ipfs://new-metadata", "URI update by new owner failed");
    }

    function testOwnerCanPauseAndUnpauseTransfers() public {
        vm.prank(owner);
        offerToken.pause();

        vm.prank(owner);
        vm.expectRevert(PausableUpgradeable.EnforcedPause.selector);
        offerToken.transfer(user, 1);

        vm.prank(owner);
        offerToken.unpause();

        vm.prank(owner);
        offerToken.transfer(user, 1);
        assertEq(offerToken.balanceOf(user), 1);
    }

    function testOnlyOwnerCanTogglePause() public {
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        vm.prank(user);
        offerToken.pause();

        vm.prank(owner);
        offerToken.pause();

        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        vm.prank(user);
        offerToken.unpause();

        vm.prank(owner);
        offerToken.unpause();
    }

    function testSnapshotCapturesBalancesAndSupply() public {
        vm.prank(owner);
        offerToken.transfer(user, 250 ether);

        vm.prank(owner);
        uint256 snapshotId = offerToken.snapshot();

        vm.prank(owner);
        offerToken.transfer(otherUser, 100 ether);

        assertEq(offerToken.balanceOfAt(user, snapshotId), 250 ether, "Snapshot balance mismatch");
        assertEq(offerToken.balanceOfAt(otherUser, snapshotId), 0, "Snapshot balance should be zero");
        assertEq(offerToken.totalSupplyAt(snapshotId), initialSupply, "Snapshot supply mismatch");
    }

    function testSnapshotIdsIncrement() public {
        vm.prank(owner);
        uint256 first = offerToken.snapshot();
        vm.prank(owner);
        uint256 second = offerToken.snapshot();

        assertEq(first + 1, second, "Snapshot ids should increment sequentially");
    }

    function testOnlyOwnerCanSnapshot() public {
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        vm.prank(user);
        offerToken.snapshot();
    }

    function testBalanceOfAtRevertsForZeroId() public {
        vm.expectRevert(bytes("Snapshot: id is 0"));
        offerToken.balanceOfAt(owner, 0);
    }

    function testBalanceOfAtRevertsForUnknownSnapshot() public {
        vm.prank(owner);
        uint256 snapshotId = offerToken.snapshot();

        vm.expectRevert(bytes("Snapshot: nonexistent id"));
        offerToken.balanceOfAt(owner, snapshotId + 1);
    }

    function testTotalSupplyAtRevertsForUnknownSnapshot() public {
        vm.prank(owner);
        uint256 snapshotId = offerToken.snapshot();

        vm.expectRevert(bytes("Snapshot: nonexistent id"));
        offerToken.totalSupplyAt(snapshotId + 1);
    }

    function testMintAndBurnUpdateSnapshots() public {
        OfferERC20Harness harnessToken = _deployHarness();

        vm.startPrank(owner);
        uint256 firstSnapshot = harnessToken.snapshot();
        harnessToken.mintTo(owner, 25);
        uint256 secondSnapshot = harnessToken.snapshot();
        harnessToken.burnFrom(owner, 5);
        uint256 thirdSnapshot = harnessToken.snapshot();
        vm.stopPrank();

        assertEq(harnessToken.totalSupplyAt(firstSnapshot), initialSupply);
        assertEq(harnessToken.totalSupplyAt(secondSnapshot), initialSupply + 25);
        assertEq(harnessToken.totalSupplyAt(thirdSnapshot), initialSupply + 20);
    }

    function testAdminTransfersRespectSnapshots() public {
        vm.prank(owner);
        uint256 beforeSnapshot = offerToken.snapshot();

        vm.prank(transferAdmin);
        offerToken.adminTransfer(owner, user, 200);

        vm.prank(owner);
        uint256 afterSnapshot = offerToken.snapshot();

        assertEq(offerToken.balanceOfAt(owner, beforeSnapshot), initialSupply);
        assertEq(offerToken.balanceOfAt(user, beforeSnapshot), 0);
        assertEq(offerToken.balanceOfAt(owner, afterSnapshot), initialSupply - 200);
        assertEq(offerToken.balanceOfAt(user, afterSnapshot), 200);
        assertEq(offerToken.totalSupplyAt(beforeSnapshot), initialSupply);
        assertEq(offerToken.totalSupplyAt(afterSnapshot), initialSupply);
    }

    function testTransferAdminAddressExposed() public {
        assertEq(offerToken.transferAdmin(), transferAdmin);
    }

    function testTransferAdminCanForceTransferWhilePaused() public {
        vm.prank(owner);
        offerToken.transfer(user, 100);

        vm.prank(owner);
        offerToken.pause();

        vm.prank(transferAdmin);
        offerToken.adminTransfer(user, otherUser, 60);

        assertEq(offerToken.balanceOf(user), 40);
        assertEq(offerToken.balanceOf(otherUser), 60);
    }

    function testTransferAdminBypassesWhitelistRestrictions() public {
        vm.prank(owner);
        offerToken.transfer(user, 100);

        vm.prank(transferAdmin);
        offerToken.adminTransfer(user, otherUser, 25);

        assertEq(offerToken.balanceOf(user), 75);
        assertEq(offerToken.balanceOf(otherUser), 25);
    }

    function testNonAdminCannotForceTransfer() public {
        vm.expectRevert(abi.encodeWithSelector(OfferERC20.NotTransferAdmin.selector, user));
        vm.prank(user);
        offerToken.adminTransfer(owner, user, 1);
    }

    function testAdminTransferRejectsZeroEndpoints() public {
        vm.expectRevert(abi.encodeWithSelector(OfferERC20.InvalidAdminTransferEndpoint.selector, address(0), user));
        vm.prank(transferAdmin);
        offerToken.adminTransfer(address(0), user, 1);

        vm.expectRevert(abi.encodeWithSelector(OfferERC20.InvalidAdminTransferEndpoint.selector, owner, address(0)));
        vm.prank(transferAdmin);
        offerToken.adminTransfer(owner, address(0), 1);
    }

    function _deployOfferToken(address[] memory initialWhitelist) private returns (OfferERC20) {
        OfferERC20 implementation = new OfferERC20();
        bytes memory initData = abi.encodeCall(
            OfferERC20.initialize,
            (TOKEN_NAME, TOKEN_SYMBOL, owner, initialSupply, INITIAL_URI, transferAdmin, initialWhitelist)
        );
        return OfferERC20(address(new ERC1967Proxy(address(implementation), initData)));
    }

    function _deployHarness() private returns (OfferERC20Harness) {
        OfferERC20Harness implementation = new OfferERC20Harness();
        bytes memory initData = abi.encodeCall(
            OfferERC20.initialize,
            (TOKEN_NAME, TOKEN_SYMBOL, owner, initialSupply, INITIAL_URI, transferAdmin, new address[](0))
        );
        return OfferERC20Harness(address(new ERC1967Proxy(address(implementation), initData)));
    }
}
