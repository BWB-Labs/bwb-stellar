// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {console2} from "forge-std/console2.sol";
import {OfferTokenSaleFactory} from "../src/OfferTokenSaleFactory.sol";
import {OfferTokenSale} from "../src/OfferTokenSale.sol";

/**
 * @title DeployOfferTokenSale
 * @notice Foundry script that uses an existing OfferTokenSaleFactory to deploy
 *         a new OfferTokenSale instance with parameters sourced from env vars.
 */
contract DeployOfferTokenSale is Script {
    struct DeploymentParams {
        address factory;
        address offerToken;
        address brlaToken;
        uint256 brlaPerToken;
        address allowlist;
        address saleOwner;
        uint256 hardCap;
        uint256 coolingOffPeriod;
        uint256 minSuccessPercent;
    }

    /**
     * @notice Deploy a new OfferTokenSale contract via a factory
     * @return sale The deployed OfferTokenSale instance
     */
    function run() external returns (OfferTokenSale sale) {
        DeploymentParams memory params = _loadParams();
        OfferTokenSaleFactory factory = OfferTokenSaleFactory(params.factory);

        vm.startBroadcast();

        address resolvedOwner = params.saleOwner;
        require(resolvedOwner != address(0), "Sale owner required");

        OfferTokenSale.SaleConfig memory saleConfig = OfferTokenSale.SaleConfig({
            hardCap: params.hardCap,
            coolingOffPeriod: params.coolingOffPeriod,
            minSuccessPercent: params.minSuccessPercent
        });

        address saleAddress = factory.createOfferTokenSale(
            params.offerToken, params.brlaToken, params.brlaPerToken, params.allowlist, resolvedOwner, saleConfig
        );

        vm.stopBroadcast();

        sale = OfferTokenSale(saleAddress);
        console2.log("OfferTokenSale deployed at:", address(sale));
        console2.log("Factory used:", address(factory));
        console2.log("Offer token:", params.offerToken);
        console2.log("BRLA token:", params.brlaToken);
        console2.log("BRLA per token (1e18):", params.brlaPerToken);
        console2.log("Allowlist:", params.allowlist);
        console2.log("Sale owner:", resolvedOwner);
        console2.log("Hard cap:", params.hardCap);
        console2.log("Cooling-off period:", params.coolingOffPeriod);
        console2.log("Min success percent:", params.minSuccessPercent);
    }

    function _loadParams() internal view returns (DeploymentParams memory params) {
        params.factory = vm.envAddress("TOKEN_SALE_FACTORY_ADDRESS");
        params.offerToken = vm.envAddress("OFFER_TOKEN_ADDRESS");
        params.brlaToken = vm.envAddress("BRLA_TOKEN_ADDRESS");
        params.brlaPerToken = vm.envUint("BRLA_PER_TOKEN");
        params.allowlist = vm.envAddress("ALLOWLIST_ADDRESS");
        params.hardCap = vm.envUint("SALE_HARD_CAP");
        params.coolingOffPeriod = _loadUintOrDefault("SALE_COOLING_OFF_PERIOD", 5 days);
        params.minSuccessPercent = _loadUintOrDefault("SALE_MIN_SUCCESS_PERCENT", 75);

        address ownerOverride = _loadAddressOrZero("SALE_OWNER");
        if (ownerOverride != address(0)) {
            params.saleOwner = ownerOverride;
        } else {
            params.saleOwner = vm.envAddress("DEPLOYER_ADDRESS");
        }
    }

    function _loadAddressOrZero(string memory key) internal view returns (address value) {
        try vm.envAddress(key) returns (address provided) {
            value = provided;
        } catch {
            value = address(0);
        }
    }

    function _loadUintOrDefault(string memory key, uint256 defaultValue) internal view returns (uint256 value) {
        try vm.envUint(key) returns (uint256 provided) {
            value = provided;
        } catch {
            value = defaultValue;
        }
    }
}
