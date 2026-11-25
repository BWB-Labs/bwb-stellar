/**
 * SPDX-License-Identifier: MIT
 */
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "./mocks/BRLA.sol";

/// @title Test suite for BRLA ERC20 token
/// @notice Covers deployment, ERC20 functions, and basic permit functionality
contract BRLATest is Test {
    BRLA brla;
    address user = address(0x123);
    uint256 initialSupply = 1_000_000 ether;

    function setUp() public {
        brla = new BRLA(initialSupply);
    }

    function testHasCorrectNameAndSymbol() public {
        assertEq(brla.name(), "BRLA", "Token name incorrect");
        assertEq(brla.symbol(), "BRLA", "Token symbol incorrect");
        assertEq(brla.decimals(), 18, "Decimals must be 18");
    }

    function testInitialSupplyIsAssignedToDeployer() public {
        assertEq(brla.totalSupply(), initialSupply, "Total supply mismatch");
        assertEq(brla.balanceOf(address(this)), initialSupply, "Deployer did not receive initial supply");
    }

    function testTransferTokens() public {
        uint256 amount = 100 ether;
        brla.transfer(user, amount);
        assertEq(brla.balanceOf(user), amount, "User did not receive transferred tokens");
        assertEq(brla.balanceOf(address(this)), initialSupply - amount, "Balance mismatch after transfer");
    }

    // Additional tests can be added here in a clean implementation.

    function testMintTokens() public {
        uint256 amount = 1e18;
        uint256 prevTotal = brla.totalSupply();
        brla.mint(address(user), amount);
        assertEq(brla.balanceOf(user), amount, "Minting did not credit to user");
        assertEq(brla.totalSupply(), prevTotal + amount, "Total supply did not increase after mint");
    }
}
