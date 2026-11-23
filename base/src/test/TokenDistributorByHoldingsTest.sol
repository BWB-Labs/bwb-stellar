// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import {ERC1967Proxy} from "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";

import {TokenDistributorByHoldings} from "../TokenDistributorByHoldings.sol";

contract MockERC20 {
    string public name;
    string public symbol;
    uint8 public decimals = 18;
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);

    constructor(string memory _name, string memory _symbol) {
        name = _name;
        symbol = _symbol;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
        emit Transfer(address(0), to, amount);
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        return transferFrom(msg.sender, to, amount);
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) public returns (bool) {
        require(balanceOf[from] >= amount, "ERC20: insufficient balance");
        if (from != msg.sender) {
            require(allowance[from][msg.sender] >= amount, "ERC20: insufficient allowance");
            allowance[from][msg.sender] -= amount;
        }
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        emit Transfer(from, to, amount);
        return true;
    }
}

contract TokenDistributorByHoldingsTest is Test {
    MockERC20 private paymentToken;
    MockERC20 private offerToken;
    TokenDistributorByHoldings private distributor;

    address private constant ALICE = address(0xA11CE);
    address private constant BOB = address(0xB0B);
    address private constant CAROL = address(0xCA901);
    address private constant RECEIVER = address(0xD15C0);

    function setUp() public {
        paymentToken = new MockERC20("BRLA", "BRLA");
        offerToken = new MockERC20("Offer", "OFR");
        TokenDistributorByHoldings implementation = new TokenDistributorByHoldings();
        bytes memory initData = abi.encodeCall(
            TokenDistributorByHoldings.initialize, (address(paymentToken), address(offerToken), address(this))
        );
        distributor = TokenDistributorByHoldings(address(new ERC1967Proxy(address(implementation), initData)));
        paymentToken.mint(address(this), 2_000 ether);
        paymentToken.approve(address(distributor), type(uint256).max);
    }

    function _mintOfferTokens() private {
        offerToken.mint(ALICE, 10 ether);
        offerToken.mint(BOB, 30 ether);
        offerToken.mint(CAROL, 60 ether);
    }

    function testHoldersCanClaimAfterDistribution() public {
        _mintOfferTokens();
        distributor.distribute(address(this), 1_000 ether);

        address[] memory holders = new address[](3);
        holders[0] = ALICE;
        holders[1] = BOB;
        holders[2] = CAROL;

        distributor.claimMany(holders);

        assertEq(paymentToken.balanceOf(ALICE), 100 ether);
        assertEq(paymentToken.balanceOf(BOB), 300 ether);
        assertEq(paymentToken.balanceOf(CAROL), 600 ether);
    }

    function testClaimForTransfersCorrectAmount() public {
        _mintOfferTokens();
        distributor.distribute(address(this), 1_000 ether);

        distributor.claimFor(ALICE);

        assertEq(paymentToken.balanceOf(ALICE), 100 ether);
        assertEq(distributor.claimable(ALICE), 0);
    }

    function testClaimableReflectsUnclaimedOwed() public {
        _mintOfferTokens();
        distributor.distribute(address(this), 1_000 ether);

        assertEq(distributor.claimable(ALICE), 100 ether);

        vm.prank(ALICE);
        distributor.claim();

        assertEq(distributor.claimable(ALICE), 0);
    }

    function testRepeatedClaimsDoNotOverpay() public {
        _mintOfferTokens();
        distributor.distribute(address(this), 1_000 ether);

        vm.prank(ALICE);
        distributor.claim();
        vm.prank(ALICE);
        distributor.claim();

        assertEq(paymentToken.balanceOf(ALICE), 100 ether);
    }

    function testWithdrawTokensTransfersEntireBalance() public {
        MockERC20 otherToken = new MockERC20("Other", "OTR");
        otherToken.mint(address(distributor), 42 ether);

        distributor.withdrawTokens(address(otherToken), RECEIVER);
        assertEq(otherToken.balanceOf(RECEIVER), 42 ether);
    }

    function testOnlyOwnerCanDistribute() public {
        vm.prank(ALICE);
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, ALICE));
        distributor.distribute(address(this), 100 ether);
    }

    function testNewOwnerCanDistribute() public {
        address newOwner = address(0xBEEF);
        distributor.transferOwnership(newOwner);

        assertEq(distributor.owner(), newOwner);

        _mintOfferTokens();
        vm.prank(newOwner);
        distributor.distribute(address(this), 500 ether);
    }

    function testTransferOwnershipZeroAddressReverts() public {
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableInvalidOwner.selector, address(0)));
        distributor.transferOwnership(address(0));
    }

    function testDistributeWithZeroAmountReverts() public {
        vm.expectRevert("Amount must be > 0");
        distributor.distribute(address(this), 0);
    }

    function testDistributeWithoutOfferSupplyReverts() public {
        vm.expectRevert("Offer token supply is zero");
        distributor.distribute(address(this), 100 ether);
    }

    function testDistributeWithoutBalanceReverts() public {
        _mintOfferTokens();
        paymentToken.transfer(address(1), paymentToken.balanceOf(address(this)));
        vm.expectRevert("ERC20: insufficient balance");
        distributor.distribute(address(this), 100 ether);
    }
}
