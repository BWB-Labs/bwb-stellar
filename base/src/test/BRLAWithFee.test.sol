// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import {BRLAWithFee} from "./mocks/BRLAWithFee.sol";

contract BRLAWithFeeHarness is BRLAWithFee {
    constructor(uint256 initialSupply) BRLAWithFee(initialSupply) {}

    function burnSelf(uint256 amount) external {
        _burn(msg.sender, amount);
    }
}

contract BRLAWithFeeTest is Test {
    BRLAWithFee token;

    address owner = address(this);
    address alice = address(0xA11CE);
    address bob = address(0xB0B);
    address spender = address(0xA1c3);

    uint256 constant INITIAL_SUPPLY = 1_000_000 ether; // 1e6 * 1e18
    uint256 constant FEE_BPS = 500; // 5%
    uint256 constant DENOM = 10_000;

    function setUp() public {
        token = new BRLAWithFee(INITIAL_SUPPLY);
    }

    function fee(uint256 amount) internal pure returns (uint256) {
        return (amount * FEE_BPS) / DENOM;
    }

    function net(uint256 amount) internal pure returns (uint256) {
        return amount - fee(amount);
    }

    // -----------------------------
    // Metadata / constructor
    // -----------------------------

    function testMetadata() public view {
        assertEq(token.name(), "BRLA-FEE");
        assertEq(token.symbol(), "BRLA-FEE");
        assertEq(token.decimals(), 18);
    }

    function testConstructorMintsToDeployer() public view {
        assertEq(token.owner(), owner);
        assertEq(token.totalSupply(), INITIAL_SUPPLY);
        assertEq(token.balanceOf(owner), INITIAL_SUPPLY);
    }

    function testFeeBasisPointsDefault() public view {
        assertEq(token.feeBasisPoints(), FEE_BPS);
    }

    // -----------------------------
    // transfer
    // -----------------------------

    function testTransferChargesFeeAndBurnsIt() public {
        uint256 amount = 100 ether;

        uint256 supplyBefore = token.totalSupply();
        uint256 ownerBefore = token.balanceOf(owner);
        uint256 bobBefore = token.balanceOf(bob);

        token.transfer(bob, amount);

        assertEq(token.balanceOf(owner), ownerBefore - amount);
        assertEq(token.balanceOf(bob), bobBefore + net(amount));
        assertEq(token.totalSupply(), supplyBefore - fee(amount));
    }

    function testTransferFeeRoundsDown() public {
        // With integer division, fee rounds down.
        uint256 amount = 1; // 1 wei, fee = floor(1*500/10000)=0
        uint256 supplyBefore = token.totalSupply();

        token.transfer(bob, amount);

        assertEq(token.balanceOf(bob), amount);
        assertEq(token.totalSupply(), supplyBefore); // no burn
    }

    function testTransferZeroAmountNoChanges() public {
        uint256 supplyBefore = token.totalSupply();
        uint256 ownerBefore = token.balanceOf(owner);
        uint256 bobBefore = token.balanceOf(bob);

        token.transfer(bob, 0);

        assertEq(token.totalSupply(), supplyBefore);
        assertEq(token.balanceOf(owner), ownerBefore);
        assertEq(token.balanceOf(bob), bobBefore);
    }

    function testTransferRevertsToZeroAddress() public {
        vm.expectRevert(bytes("ERC20: transfer to the zero address"));
        token.transfer(address(0), 1);
    }

    function testTransferRevertsInsufficientBalance() public {
        vm.prank(alice);
        vm.expectRevert(bytes("ERC20: transfer amount exceeds balance"));
        token.transfer(bob, 1);
    }

    // -----------------------------
    // approve / allowance
    // -----------------------------

    function testApproveSetsAllowance() public {
        assertEq(token.allowance(owner, spender), 0);

        bool ok = token.approve(spender, 123);
        assertTrue(ok);

        assertEq(token.allowance(owner, spender), 123);
    }

    function testApproveRevertsSpenderZero() public {
        vm.expectRevert(bytes("ERC20: approve to the zero address"));
        token.approve(address(0), 1);
    }

    // Note: approve-from-zero-address cannot happen externally because msg.sender
    // is never address(0), so no test for that path.

    // -----------------------------
    // transferFrom
    // -----------------------------

    function testTransferFromChargesFeeBurnsAndDecreasesAllowance() public {
        uint256 amount = 200 ether;

        token.transfer(alice, 1_000 ether);

        vm.prank(alice);
        token.approve(spender, amount);

        uint256 supplyBefore = token.totalSupply();
        uint256 aliceBefore = token.balanceOf(alice);
        uint256 bobBefore = token.balanceOf(bob);

        vm.prank(spender);
        bool ok = token.transferFrom(alice, bob, amount);
        assertTrue(ok);

        assertEq(token.allowance(alice, spender), 0);
        assertEq(token.balanceOf(alice), aliceBefore - amount);
        assertEq(token.balanceOf(bob), bobBefore + net(amount));
        assertEq(token.totalSupply(), supplyBefore - fee(amount));
    }

    function testTransferFromRevertsInsufficientAllowance() public {
        token.transfer(alice, 10 ether);

        vm.prank(spender);
        vm.expectRevert(bytes("ERC20: transfer amount exceeds allowance"));
        token.transferFrom(alice, bob, 1 ether);
    }

    function testTransferFromRevertsToZeroAddress() public {
        token.transfer(alice, 10 ether);

        vm.prank(alice);
        token.approve(spender, 1 ether);

        vm.prank(spender);
        vm.expectRevert(bytes("ERC20: transfer to the zero address"));
        token.transferFrom(alice, address(0), 1 ether);
    }

    function testTransferFromRevertsInsufficientBalanceEvenWithAllowance() public {
        vm.prank(alice);
        token.approve(spender, 10 ether);

        vm.prank(spender);
        vm.expectRevert(bytes("ERC20: transfer amount exceeds balance"));
        token.transferFrom(alice, bob, 1 ether);
    }

    function testTransferFromZeroAmountStillConsumesNoSupplyButConsumesAllowance() public {
        token.transfer(alice, 1 ether);

        vm.prank(alice);
        token.approve(spender, 123);

        uint256 supplyBefore = token.totalSupply();

        vm.prank(spender);
        token.transferFrom(alice, bob, 0);

        assertEq(token.totalSupply(), supplyBefore); // no burn
        assertEq(token.allowance(alice, spender), 123); // allowance reduced by 0
        assertEq(token.balanceOf(bob), 0); // received 0
    }

    // -----------------------------
    // Invariants / consistency
    // -----------------------------

    function testSumOfBalancesPlusBurnedFeeEqualsInitialSupplyAfterTransfers() public {
        // This contract burns fees by reducing totalSupply but does not track burned balance.
        // We can check: totalSupply + (fees burned) == initialSupply, if we compute fees.
        uint256 a1 = 100 ether;
        uint256 a2 = 333 ether;
        uint256 a3 = 1; // no fee burn

        uint256 burned = 0;

        token.transfer(alice, a1);
        burned += fee(a1);

        token.transfer(bob, a2);
        burned += fee(a2);

        token.transfer(bob, a3);
        burned += fee(a3);

        assertEq(token.totalSupply() + burned, INITIAL_SUPPLY);
    }

    function testBurnSelfReducesSupply() public {
        BRLAWithFeeHarness harness = new BRLAWithFeeHarness(INITIAL_SUPPLY);
        uint256 supplyBefore = harness.totalSupply();
        uint256 burnAmount = 1_000 ether;

        harness.burnSelf(burnAmount);

        assertEq(harness.totalSupply(), supplyBefore - burnAmount);
        assertEq(harness.balanceOf(address(this)), supplyBefore - burnAmount);
    }

    function testBurnSelfRevertsOnInsufficientBalance() public {
        BRLAWithFeeHarness harness = new BRLAWithFeeHarness(INITIAL_SUPPLY);
        vm.expectRevert(bytes("ERC20: burn amount exceeds balance"));
        harness.burnSelf(INITIAL_SUPPLY + 1);
    }
}
