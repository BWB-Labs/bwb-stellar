// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "./mocks/MockUSDC.sol";

/// @title Test suite for MockUSDC ERC20 token
/// @notice Covers deployment, ERC20 functions, and transfer validations
contract MockUSDCTest is Test {
    MockUSDC mockUSDC;
    address user1 = address(0x123);
    address user2 = address(0x456);
    uint256 initialSupply = 1_000_000 ether;

    function setUp() public {
        mockUSDC = new MockUSDC(initialSupply);
    }

    function testHasCorrectMetadata() public {
        assertEq(mockUSDC.name(), "Mock USDC", "Token name incorrect");
        assertEq(mockUSDC.symbol(), "mUSDC", "Token symbol incorrect");
        assertEq(mockUSDC.decimals(), 18, "Decimals must be 18");
    }

    function testInitialSupplyMintedToDeployer() public {
        assertEq(mockUSDC.totalSupply(), initialSupply, "Total supply mismatch");
        assertEq(mockUSDC.balanceOf(address(this)), initialSupply, "Deployer did not receive initial supply");
    }

    function testTransferTokens() public {
        uint256 amount = 100 ether;
        mockUSDC.transfer(user1, amount);
        assertEq(mockUSDC.balanceOf(user1), amount, "User1 did not receive transferred tokens");
        assertEq(mockUSDC.balanceOf(address(this)), initialSupply - amount, "Balance mismatch after transfer");
    }

    function testTransferToMultipleUsers() public {
        uint256 amount1 = 200 ether;
        uint256 amount2 = 300 ether;

        mockUSDC.transfer(user1, amount1);
        mockUSDC.transfer(user2, amount2);

        assertEq(mockUSDC.balanceOf(user1), amount1, "User1 balance incorrect");
        assertEq(mockUSDC.balanceOf(user2), amount2, "User2 balance incorrect");
        assertEq(mockUSDC.balanceOf(address(this)), initialSupply - amount1 - amount2, "Deployer balance incorrect");
    }

    function testCannotTransferToZeroAddress() public {
        uint256 amount = 100 ether;
        vm.expectRevert("transfer to zero address");
        mockUSDC.transfer(address(0), amount);
    }

    function testCannotTransferWithoutBalance() public {
        vm.prank(user1);
        vm.expectRevert("insufficient balance");
        mockUSDC.transfer(user2, 1 ether);
    }
}
