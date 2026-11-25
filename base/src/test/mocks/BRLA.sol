// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {ERC20Permit} from "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";

/// @title BRLA Token
/// @notice Simple ERC20 token named BRLA that mints the initial supply to the deployer.
contract BRLA is ERC20, ERC20Permit {
    constructor(uint256 initialSupply) ERC20("BRLA", "BRLA") ERC20Permit("BRLA") {
        _mint(msg.sender, initialSupply);
    }

    /**
     * @notice Test helper that allows anyone to mint arbitrary BRLA.
     * @param to Recipient address.
     * @param amount Amount of BRLA (18 decimals) to mint.
     */
    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}
