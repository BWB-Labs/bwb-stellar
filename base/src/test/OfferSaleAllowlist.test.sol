// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import "../OfferSaleAllowlist.sol";

contract DummyController {
    function ping() external pure returns (uint256) {
        return 1;
    }
}

contract OfferSaleAllowlistTest is Test {
    OfferSaleAllowlist allowlist;

    address owner = address(this);
    address controller = address(0x1);
    address controller2 = address(0x2);
    address buyer1 = address(0x3);
    address buyer2 = address(0x4);

    error OwnableUnauthorizedAccount(address account);

    function setUp() public {
        OfferSaleAllowlist implementation = new OfferSaleAllowlist();
        bytes memory initData = abi.encodeCall(OfferSaleAllowlist.initialize, (owner));
        allowlist = OfferSaleAllowlist(address(new ERC1967Proxy(address(implementation), initData)));
    }

    function _setBuyer(address _buyer, uint256 _max, bool _enabled) internal {
        bool prevControllerState = allowlist.controllers(address(this));
        if (!prevControllerState) {
            allowlist.setController(address(this), true);
        }

        address[] memory buyers = new address[](1);
        uint256[] memory maxes = new uint256[](1);
        bool[] memory enableds = new bool[](1);

        buyers[0] = _buyer;
        maxes[0] = _max;
        enableds[0] = _enabled;

        allowlist.setAllocations(buyers, maxes, enableds);

        if (!prevControllerState) {
            allowlist.setController(address(this), false);
        }
    }

    function _enableTemporaryController() internal returns (bool wasEnabled) {
        wasEnabled = allowlist.controllers(address(this));
        if (!wasEnabled) {
            allowlist.setController(address(this), true);
        }
    }

    function _restoreControllerState(bool wasEnabled) internal {
        if (!wasEnabled) {
            allowlist.setController(address(this), false);
        }
    }

    function _enableController(address c) internal {
        allowlist.setController(c, true);
    }

    function testSetAllocationsSuccess() public {
        _setBuyer(buyer1, 1000 ether, true);
        assertEq(allowlist.remainingAllocation(buyer1), 1000 ether);
    }

    function testSetAllocationsArrayLengthMismatch() public {
        address[] memory buyers = new address[](1);
        uint256[] memory max = new uint256[](2);
        bool[] memory enabled = new bool[](1);

        _enableTemporaryController();
        vm.expectRevert("Array length mismatch");
        allowlist.setAllocations(buyers, max, enabled);
    }

    function testSetAllocationsZeroBuyer() public {
        address[] memory buyers = new address[](1);
        uint256[] memory max = new uint256[](1);
        bool[] memory enabled = new bool[](1);

        buyers[0] = address(0);
        max[0] = 1;
        enabled[0] = true;

        _enableTemporaryController();
        vm.expectRevert("Invalid buyer");
        allowlist.setAllocations(buyers, max, enabled);

        // Removed redundant call with uninitialized 'buyer' array
    }

    function testSetAllocationsRequiresController() public {
        address[] memory buyers = new address[](1);
        buyers[0] = buyer1;
        uint256[] memory maxes = new uint256[](1);
        maxes[0] = 1;
        bool[] memory enabled = new bool[](1);
        enabled[0] = true;

        vm.prank(address(0xdead));
        vm.expectRevert("Unauthorized controller");
        allowlist.setAllocations(buyers, maxes, enabled);
    }

    function testSetControllerEnableDisable() public {
        allowlist.setController(controller, true);
        assertTrue(allowlist.controllers(controller));

        allowlist.setController(controller, false);
        assertFalse(allowlist.controllers(controller));
    }

    function testSetMultipleControllers() public {
        allowlist.setController(controller, true);
        allowlist.setController(controller2, true);

        _setBuyer(buyer1, 1000 ether, true);

        vm.prank(controller);
        allowlist.consumeAllocation(buyer1, 100 ether);

        vm.prank(controller2);
        allowlist.consumeAllocation(buyer1, 100 ether);

        assertEq(allowlist.remainingAllocation(buyer1), 800 ether);
    }

    function testSetControllerZeroAddress() public {
        vm.expectRevert("Invalid controller");
        allowlist.setController(address(0), true);
    }

    function testSetControllerRequiresOwner() public {
        vm.expectRevert(abi.encodeWithSelector(OwnableUnauthorizedAccount.selector, address(0xdead)));
        vm.prank(address(0xdead));
        allowlist.setController(controller, true);
    }

    function testSetControllerByOwner() public {
        allowlist.setController(controller, true);
        assertTrue(allowlist.controllers(controller));
    }

    function testSetControllerFromControllerAllowsEOA() public {
        allowlist.setController(controller, true);

        vm.prank(controller);
        allowlist.setControllerFromController(controller2, true);

        assertTrue(allowlist.controllers(controller2));
    }

    function testSetControllerFromControllerRevokes() public {
        allowlist.setController(controller, true);
        allowlist.setController(controller2, true);

        vm.prank(controller);
        allowlist.setControllerFromController(controller2, false);

        assertFalse(allowlist.controllers(controller2));
    }

    function testSetControllerFromControllerAllowsContracts() public {
        allowlist.setController(controller, true);
        DummyController dummy = new DummyController();

        vm.prank(controller);
        allowlist.setControllerFromController(address(dummy), true);

        assertTrue(allowlist.controllers(address(dummy)));
    }

    function testSetControllerFromControllerRejectsZeroAddress() public {
        allowlist.setController(controller, true);

        vm.prank(controller);
        vm.expectRevert("Invalid controller");
        allowlist.setControllerFromController(address(0), true);
    }

    function testSetControllerFromControllerRequiresControllerCaller() public {
        vm.expectRevert("Unauthorized controller");
        allowlist.setControllerFromController(controller2, true);
    }

    /* ---------- consumeAllocation ---------- */

    function testConsumeAllocationSuccessAndExceed() public {
        _setBuyer(buyer1, 1000 ether, true);
        _enableController(controller);

        vm.prank(controller);
        allowlist.consumeAllocation(buyer1, 900 ether);

        vm.prank(controller);
        vm.expectRevert("Allocation exceeded");
        allowlist.consumeAllocation(buyer1, 200 ether);
    }

    function testConsumeAllocationNotEnabled() public {
        _setBuyer(buyer1, 1000 ether, false);
        _enableController(controller);

        vm.prank(controller);
        vm.expectRevert("Buyer not whitelisted");
        allowlist.consumeAllocation(buyer1, 1);
    }

    function testConsumeAllocationUnauthorized() public {
        _setBuyer(buyer1, 1000 ether, true);

        vm.expectRevert("Unauthorized controller");
        allowlist.consumeAllocation(buyer1, 1);
    }

    /* ---------- canPurchase ---------- */

    function testCanPurchaseBranches() public {
        _setBuyer(buyer1, 1000 ether, true);

        assertTrue(allowlist.canPurchase(buyer1, 500 ether));
        assertFalse(allowlist.canPurchase(buyer1, 1500 ether));
    }

    function testCanPurchaseDisabled() public {
        _setBuyer(buyer1, 1000 ether, false);
        assertFalse(allowlist.canPurchase(buyer1, 1));
    }

    /* ---------- resetConsumed ---------- */

    function testResetConsumedRequiresController() public {
        address[] memory buyers = new address[](1);
        buyers[0] = buyer1;

        vm.prank(address(0xdead));
        vm.expectRevert("Unauthorized controller");
        allowlist.resetConsumed(buyers);
    }

    function testRemainingAllocationWhenDisabled() public {
        _setBuyer(buyer1, 1000 ether, false);
        assertEq(allowlist.remainingAllocation(buyer1), 0);
    }

    function testSetAllocations() public {
        address[] memory buyers = new address[](2);
        uint256[] memory maxes = new uint256[](2);
        bool[] memory enabled = new bool[](2);

        buyers[0] = buyer1;
        buyers[1] = buyer2;
        maxes[0] = 1000 ether;
        maxes[1] = 2000 ether;
        enabled[0] = true;
        enabled[1] = false;

        bool prevController = _enableTemporaryController();
        allowlist.setAllocations(buyers, maxes, enabled);
        _restoreControllerState(prevController);
        assertEq(allowlist.remainingAllocation(buyer1), 1000 ether);
        assertEq(allowlist.remainingAllocation(buyer2), 0);
    }

    function testControllerAuthorization() public {
        allowlist.setController(controller, true);
        assertTrue(allowlist.controllers(controller));
    }

    function testConsumeAllocation() public {
        address[] memory buyers = new address[](1);
        uint256[] memory maxes = new uint256[](1);
        bool[] memory enabled = new bool[](1);

        buyers[0] = buyer1;
        maxes[0] = 1000 ether;
        enabled[0] = true;

        bool prevController = _enableTemporaryController();
        allowlist.setAllocations(buyers, maxes, enabled);
        _restoreControllerState(prevController);
        allowlist.setController(controller, true);
        vm.prank(controller);
        allowlist.consumeAllocation(buyer1, 300 ether);
        assertEq(allowlist.remainingAllocation(buyer1), 700 ether);
    }

    function testUnauthorizedController() public {
        address[] memory buyers = new address[](1);
        uint256[] memory maxes = new uint256[](1);
        bool[] memory enabled = new bool[](1);

        buyers[0] = buyer1;
        maxes[0] = 1000 ether;
        enabled[0] = true;

        bool prevController = _enableTemporaryController();
        allowlist.setAllocations(buyers, maxes, enabled);
        _restoreControllerState(prevController);
        vm.prank(address(0x6));
        vm.expectRevert("Unauthorized controller");
        allowlist.consumeAllocation(buyer1, 100 ether);
    }

    function testResetConsumed() public {
        address[] memory buyers = new address[](1);
        uint256[] memory maxes = new uint256[](1);
        bool[] memory enabled = new bool[](1);

        buyers[0] = buyer1;
        maxes[0] = 1000 ether;
        enabled[0] = true;

        bool prevController = _enableTemporaryController();
        allowlist.setAllocations(buyers, maxes, enabled);
        _restoreControllerState(prevController);
        allowlist.setController(controller, true);
        vm.prank(controller);
        allowlist.consumeAllocation(buyer1, 600 ether);

        address[] memory resetBuyers = new address[](1);
        resetBuyers[0] = buyer1;
        prevController = _enableTemporaryController();
        allowlist.resetConsumed(resetBuyers);
        _restoreControllerState(prevController);
        assertEq(allowlist.remainingAllocation(buyer1), 1000 ether);
    }
}
