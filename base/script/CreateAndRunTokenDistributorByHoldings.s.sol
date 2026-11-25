// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {TokenDistributorByHoldingsFactory} from "../src/TokenDistributorByHoldingsFactory.sol";
import {TokenDistributorByHoldings} from "../src/TokenDistributorByHoldings.sol";

/**
 * @title CreateAndRunTokenDistributorByHoldings
 * @notice Deploys a TokenDistributorByHoldings via its factory, funds it with BRLA, and executes a distribution.
 */
contract CreateAndRunTokenDistributorByHoldings is Script {
    /**
     * @notice Reads configuration from env vars, deploys a distributor, funds it, and runs a proportional distribution.
     */
    function run() external {
        address factoryAddress = vm.envAddress("TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS");
        address brlaTokenAddress = vm.envAddress("BRLA_TOKEN_ADDRESS");
        address offerTokenAddress = vm.envAddress("OFFER_TOKEN_ADDRESS");
        address ownerAddress = vm.envAddress("DEPLOYER_ADDRESS");
        uint256 totalBrla = vm.envUint("BRLA_DISTRIBUTION_TOTAL");
        string memory holdersJson = vm.envString("HOLDERS_JSON");
        address[] memory holders = abi.decode(vm.parseJson(holdersJson), (address[]));
        require(holders.length > 0, "Holders required");

        TokenDistributorByHoldingsFactory factory = TokenDistributorByHoldingsFactory(factoryAddress);
        IERC20 brla = IERC20(brlaTokenAddress);

        uint256[] memory balancesBefore = new uint256[](holders.length);
        for (uint256 i = 0; i < holders.length; ++i) {
            balancesBefore[i] = brla.balanceOf(holders[i]);
        }

        vm.startBroadcast();
        address distributorAddress = factory.createDistributor(ownerAddress, brlaTokenAddress, offerTokenAddress);
        console2.log("TokenDistributorByHoldings deployed at:", distributorAddress);

        brla.approve(distributorAddress, totalBrla);
        console2.log("Funded distributor with", totalBrla, "wei BRLA");

        TokenDistributorByHoldings distributor = TokenDistributorByHoldings(distributorAddress);
        distributor.distribute(msg.sender, totalBrla);
        distributor.claimMany(holders);
        vm.stopBroadcast();

        bool equalShares = true;
        uint256 referenceShare = 0;

        for (uint256 i = 0; i < holders.length; ++i) {
            uint256 afterBalance = brla.balanceOf(holders[i]);
            uint256 distributed = afterBalance - balancesBefore[i];
            console2.log("Holder", holders[i]);
            console2.log("Distributed BRLA (wei)", distributed);
            if (i == 0) {
                referenceShare = distributed;
            } else if (distributed != referenceShare) {
                equalShares = false;
            }
        }

        console2.log("Equal distribution across holders:", equalShares);
    }
}
