// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../../TokenDistributor.sol";

/// @custom:oz-upgrades-unsafe-allow constructor
contract TokenDistributorV2 is TokenDistributor {
    constructor() {
        _disableInitializers();
    }

    function versionString() external pure returns (string memory) {
        return "2.0.0";
    }

    function initialize(address owner_, string calldata name_, string calldata descriptionURI_)
        external
        override
        initializer
    {
        __TokenDistributor_init(owner_, name_, descriptionURI_);
    }

    function setDescriptionURI(string calldata newDescription) external onlyOwner {
        descriptionURI = newDescription;
    }
}
