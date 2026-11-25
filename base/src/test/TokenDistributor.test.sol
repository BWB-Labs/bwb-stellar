// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/mocks/token/ERC20Mock.sol";
import "openzeppelin-contracts/contracts/proxy/Clones.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";

import "../TokenDistributor.sol";

contract TokenDistributorTest is Test {
    event MetadataUpdated(string name, string descriptionURI);
    event Initialized(address owner, string name, string descriptionURI);
    event TokensDistributed(address indexed token, uint256 totalAmount, uint256 recipientCount);
    event RecipientTokensDistributed(
        address indexed token, address indexed recipient, uint256 amount, uint256 distributorBalance
    );

    TokenDistributor private implementation;
    address private ownerAddr;

    function setUp() public {
        implementation = new TokenDistributor();
        ownerAddr = makeAddr("owner");
    }

    function testDirectDeploymentInitializationRevert() public {
        TokenDistributor direct = new TokenDistributor();
        assertEq(direct.owner(), address(0));
        vm.expectRevert(abi.encodeWithSelector(Initializable.InvalidInitialization.selector));
        direct.initialize(address(0x1), "Name", "Desc");
    }

    function testCloneInitializeSetsStateAndEmits() public {
        TokenDistributor clone = _deployClone();
        string memory name = "My Distributor";
        string memory uri = "ipfs://metadata.json";

        vm.expectEmit(false, false, false, true, address(clone));
        emit Initialized(ownerAddr, name, uri);
        clone.initialize(ownerAddr, name, uri);

        assertEq(clone.owner(), ownerAddr);
        assertEq(clone.name(), name);
        assertEq(clone.descriptionURI(), uri);
    }

    function testCloneInitializeOnlyOnce() public {
        TokenDistributor clone = _deployClone();
        clone.initialize(ownerAddr, "First", "first://uri");

        vm.expectRevert(abi.encodeWithSelector(Initializable.InvalidInitialization.selector));
        clone.initialize(ownerAddr, "Second", "second://uri");
    }

    function testCloneInitializeZeroOwnerReverts() public {
        TokenDistributor clone = _deployClone();

        vm.expectRevert("Owner cannot be zero address");
        clone.initialize(address(0), "Name", "Desc");
    }

    function testUpdateMetadataOnlyOwner() public {
        TokenDistributor clone = _deployInitializedClone();
        string memory newName = "Updated";
        string memory newUri = "https://example.com/details";

        vm.expectEmit(false, false, false, true, address(clone));
        emit MetadataUpdated(newName, newUri);

        vm.prank(ownerAddr);
        clone.updateMetadata(newName, newUri);

        assertEq(clone.name(), newName);
        assertEq(clone.descriptionURI(), newUri);
    }

    function testUpdateMetadataNonOwnerReverts() public {
        TokenDistributor clone = _deployInitializedClone();

        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, address(this)));
        clone.updateMetadata("Name", "uri");
    }

    function testDistributeTokensSuccess() public {
        TokenDistributor clone = _deployInitializedClone();
        ERC20Mock token = new ERC20Mock();

        address[] memory recipients = new address[](2);
        recipients[0] = makeAddr("recipient1");
        recipients[1] = makeAddr("recipient2");

        uint256[] memory amounts = new uint256[](2);
        amounts[0] = 2 ether;
        amounts[1] = 3 ether;

        uint256 totalAmount = amounts[0] + amounts[1];
        token.mint(address(clone), totalAmount);

        vm.expectEmit(true, true, false, true, address(clone));
        emit RecipientTokensDistributed(address(token), recipients[0], amounts[0], totalAmount - amounts[0]);
        vm.expectEmit(true, true, false, true, address(clone));
        emit RecipientTokensDistributed(address(token), recipients[1], amounts[1], 0);
        vm.expectEmit(true, false, false, true, address(clone));
        emit TokensDistributed(address(token), totalAmount, recipients.length);

        vm.prank(ownerAddr);
        clone.distributeTokens(address(token), recipients, amounts);

        assertEq(token.balanceOf(address(clone)), 0);
        assertEq(token.balanceOf(recipients[0]), amounts[0]);
        assertEq(token.balanceOf(recipients[1]), amounts[1]);
    }

    function testDistributeTokensNonOwnerReverts() public {
        TokenDistributor clone = _deployInitializedClone();
        ERC20Mock token = new ERC20Mock();

        address[] memory recipients = new address[](1);
        recipients[0] = makeAddr("recipient");
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1 ether;

        token.mint(address(clone), amounts[0]);

        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, address(this)));
        clone.distributeTokens(address(token), recipients, amounts);
    }

    function testDistributeTokensMismatchedArraysReverts() public {
        TokenDistributor clone = _deployInitializedClone();
        ERC20Mock token = new ERC20Mock();

        address[] memory recipients = new address[](2);
        recipients[0] = makeAddr("recipient1");
        recipients[1] = makeAddr("recipient2");

        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1 ether;

        vm.prank(ownerAddr);
        vm.expectRevert("Recipients and amounts mismatch");
        clone.distributeTokens(address(token), recipients, amounts);
    }

    function testDistributeTokensInvalidTokenAddressReverts() public {
        TokenDistributor clone = _deployInitializedClone();

        address[] memory recipients = new address[](1);
        recipients[0] = makeAddr("recipient");
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1 ether;

        vm.prank(ownerAddr);
        vm.expectRevert("Invalid token address");
        clone.distributeTokens(address(0), recipients, amounts);
    }

    function testDistributeTokensZeroRecipientReverts() public {
        TokenDistributor clone = _deployInitializedClone();
        ERC20Mock token = new ERC20Mock();

        address[] memory recipients = new address[](1);
        recipients[0] = address(0);
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1 ether;

        token.mint(address(clone), amounts[0]);

        vm.prank(ownerAddr);
        vm.expectRevert("Cannot transfer to zero address");
        clone.distributeTokens(address(token), recipients, amounts);
    }

    function testDistributeTokensInsufficientBalanceReverts() public {
        TokenDistributor clone = _deployInitializedClone();
        ERC20Mock token = new ERC20Mock();

        address[] memory recipients = new address[](1);
        recipients[0] = makeAddr("recipient");
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 5 ether;

        token.mint(address(clone), 1 ether);

        vm.prank(ownerAddr);
        vm.expectRevert("Insufficient token balance");
        clone.distributeTokens(address(token), recipients, amounts);
    }

    function testDistributeTokensBatchSizeLimitReverts() public {
        TokenDistributor clone = _deployInitializedClone();
        ERC20Mock token = new ERC20Mock();

        address[] memory recipients = new address[](201);
        uint256[] memory amounts = new uint256[](201);
        for (uint256 i = 0; i < recipients.length; ++i) {
            recipients[i] = address(uint160(i + 1));
            amounts[i] = 1;
        }

        vm.prank(ownerAddr);
        vm.expectRevert("Batch size exceeds limit");
        clone.distributeTokens(address(token), recipients, amounts);
    }

    function _deployClone() private returns (TokenDistributor) {
        address clone = Clones.clone(address(implementation));
        return TokenDistributor(clone);
    }

    function _deployInitializedClone() private returns (TokenDistributor) {
        TokenDistributor clone = _deployClone();
        clone.initialize(ownerAddr, "Initial", "initial://uri");
        return clone;
    }
}
