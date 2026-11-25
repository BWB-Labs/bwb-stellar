// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {TokenDistributorFactory} from "../src/TokenDistributorFactory.sol";
import {TokenDistributor} from "../src/TokenDistributor.sol";

/**
 * @title CreateAndRunTokenDistributor
 * @notice Creates a TokenDistributor clone, funds it with BRLA, and splits the payout between advisor and deployer.
 */
contract CreateAndRunTokenDistributor is Script {
    /**
     * @notice Executes the full distribution workflow using env-provided configuration.
     */
    function run() external {
        address factoryAddress = vm.envAddress("TOKEN_DISTRIBUTOR_FACTORY_ADDRESS");
        address brlaTokenAddress = vm.envAddress("BRLA_TOKEN_ADDRESS");
        address ownerAddress = vm.envAddress("DEPLOYER_ADDRESS");
        address advisorAddress = vm.envAddress("ADVISOR_ADDRESS");
        uint256 totalBrla = vm.envUint("BRLA_DISTRIBUTION_TOTAL");
        require(totalBrla > 0, "Distribution amount must be > 0");

        string memory distributorName = _envStringOrDefault("TOKEN_DISTRIBUTOR_NAME", "Advisor/Deployer Distribution");
        string memory distributorDescription =
            _envStringOrDefault("TOKEN_DISTRIBUTOR_DESCRIPTION", "One-off BRLA split between advisor and deployer");

        TokenDistributorFactory factory = TokenDistributorFactory(factoryAddress);

        vm.startBroadcast();
        address distributor = factory.createTokenDistributor(ownerAddress, distributorName, distributorDescription);
        console2.log("TokenDistributor deployed at", distributor);

        IERC20 brla = IERC20(brlaTokenAddress);
        brla.transfer(distributor, totalBrla);
        console2.log("Funded distributor with", totalBrla);

        address[] memory recipients = new address[](2);
        recipients[0] = advisorAddress;
        recipients[1] = ownerAddress;

        uint256[] memory amounts = new uint256[](2);
        uint256 half = totalBrla / 2;
        amounts[0] = half;
        amounts[1] = totalBrla - half;

        TokenDistributor(distributor).distributeTokens(brlaTokenAddress, recipients, amounts);
        console2.log("Distributed BRLA to advisor and deployer");
        vm.stopBroadcast();

        console2.log("Token distribution complete");
    }

    function _envStringOrDefault(string memory key, string memory fallbackValue) private view returns (string memory) {
        try vm.envString(key) returns (string memory value) {
            return value;
        } catch {
            return fallbackValue;
        }
    }
}
