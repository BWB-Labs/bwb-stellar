// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "../../TokenDistributorByHoldings.sol";

/// @custom:oz-upgrades-unsafe-allow constructor
contract TokenDistributorByHoldingsV2 is TokenDistributorByHoldings {
    constructor() {
        _disableInitializers();
    }

    function versionString() external pure returns (string memory) {
        return "2.0.0";
    }

    function bumpCumulativePerToken(uint256 increment) external onlyOwner {
        cumulativePerToken += increment;
    }

    function initialize(address brlaToken_, address offerToken_, address owner_) external override initializer {
        __TokenDistributorByHoldings_init(brlaToken_, offerToken_, owner_);
    }
}
