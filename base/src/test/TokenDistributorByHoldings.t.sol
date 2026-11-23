// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";

import {ERC1967Proxy} from "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

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

contract TokenDistributorByHoldingsAccessTest is Test {
    MockERC20 private paymentToken;
    MockERC20 private offerToken;
    TokenDistributorByHoldings private distributor;

    address private constant HOLDER = address(0xDEAD);

    function setUp() public {
        paymentToken = new MockERC20("BRLA", "BRLA");
        offerToken = new MockERC20("Offer", "OFR");
        TokenDistributorByHoldings implementation = new TokenDistributorByHoldings();
        bytes memory initData = abi.encodeCall(
            TokenDistributorByHoldings.initialize, (address(paymentToken), address(offerToken), address(this))
        );
        distributor = TokenDistributorByHoldings(address(new ERC1967Proxy(address(implementation), initData)));
        paymentToken.mint(address(this), 1_000 ether);
        paymentToken.approve(address(distributor), type(uint256).max);
    }

    function testClaimManyRequiresHolders() public {
        vm.expectRevert("No holders provided");
        distributor.claimMany(new address[](0));
    }

    function testClaimForZeroHolderReverts() public {
        vm.expectRevert("Holder cannot be zero address");
        distributor.claimFor(address(0));
    }

    function testWithdrawTokensInvalidArgsRevert() public {
        vm.expectRevert("Invalid token address");
        distributor.withdrawTokens(address(0), address(this));

        vm.expectRevert("Invalid recipient address");
        distributor.withdrawTokens(address(paymentToken), address(0));
    }

    function testClaimableZeroBeforeDistribution() public {
        assertEq(distributor.claimable(HOLDER), 0);
    }

    function testDistributionFailsWithoutSupply() public {
        vm.expectRevert("Offer token supply is zero");
        distributor.distribute(address(this), 1 ether);
    }

    function testClaimDoesNothingForHolderWithoutHoldings() public {
        // Fund supply but holder has zero balance
        offerToken.mint(address(1), 1 ether);
        distributor.distribute(address(this), 100 ether);

        vm.prank(HOLDER);
        distributor.claim();
        assertEq(paymentToken.balanceOf(HOLDER), 0);
    }
}
