// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";

/**
 * @title SafeAddressRegistry
 * @notice Canonical registry for BWBR tokenization contracts. Own this with the
 *         project Gnosis Safe and update it immediately after deploying or
 *         upgrading factories, implementations, or proxies. Downstream
 *         services should reference the stored addresses to ensure they always
 *         target the approved deployments.
 */
contract SafeAddressRegistry is Ownable {
    error ZeroAddress();
    error UnchangedAddress();
    error AddressAlreadyTracked();
    error AddressNotTracked();

    event OfferTokenImplementationUpdated(address indexed implementation);
    event OfferTokenBeaconUpdated(address indexed beacon);
    event OfferTokenFactoryUpdated(address indexed factory);
    event OfferTokenProxyRegistered(address indexed proxy);
    event OfferTokenProxyRemoved(address indexed proxy);

    event OfferTokenSaleImplementationUpdated(address indexed implementation);
    event OfferTokenSaleBeaconUpdated(address indexed beacon);
    event OfferTokenSaleFactoryUpdated(address indexed factory);
    event OfferTokenSaleProxyRegistered(address indexed proxy);
    event OfferTokenSaleProxyRemoved(address indexed proxy);

    event TokenDistributorImplementationUpdated(address indexed implementation);
    event TokenDistributorBeaconUpdated(address indexed beacon);
    event TokenDistributorFactoryUpdated(address indexed factory);
    event TokenDistributorProxyRegistered(address indexed proxy);
    event TokenDistributorProxyRemoved(address indexed proxy);

    event TokenDistributorByHoldingsImplementationUpdated(address indexed implementation);
    event TokenDistributorByHoldingsBeaconUpdated(address indexed beacon);
    event TokenDistributorByHoldingsFactoryUpdated(address indexed factory);
    event TokenDistributorByHoldingsProxyRegistered(address indexed proxy);
    event TokenDistributorByHoldingsProxyRemoved(address indexed proxy);

    event OfferSaleAllowlistUpdated(address indexed allowlist);

    address public offerTokenImplementation;
    address public offerTokenBeacon;
    address public offerTokenFactory;
    address[] private offerTokenProxies;

    address public offerTokenSaleImplementation;
    address public offerTokenSaleBeacon;
    address public offerTokenSaleFactory;
    address[] private offerTokenSaleProxies;

    address public tokenDistributorImplementation;
    address public tokenDistributorBeacon;
    address public tokenDistributorFactory;
    address[] private tokenDistributorProxies;

    address public tokenDistributorByHoldingsImplementation;
    address public tokenDistributorByHoldingsBeacon;
    address public tokenDistributorByHoldingsFactory;
    address[] private tokenDistributorByHoldingsProxies;

    address public offerSaleAllowlist;

    constructor(address safe) Ownable(safe) {
        if (safe == address(0)) {
            revert ZeroAddress();
        }
    }

    // Offer Token ---------------------------------------------------------

    function setOfferTokenImplementation(address newImplementation) external onlyOwner {
        _validateNewAddress(offerTokenImplementation, newImplementation);
        offerTokenImplementation = newImplementation;
        emit OfferTokenImplementationUpdated(newImplementation);
    }

    function setOfferTokenBeacon(address newBeacon) external onlyOwner {
        _validateNewAddress(offerTokenBeacon, newBeacon);
        offerTokenBeacon = newBeacon;
        emit OfferTokenBeaconUpdated(newBeacon);
    }

    function setOfferTokenFactory(address newFactory) external onlyOwner {
        _validateNewAddress(offerTokenFactory, newFactory);
        offerTokenFactory = newFactory;
        emit OfferTokenFactoryUpdated(newFactory);
    }

    function addOfferTokenProxy(address proxy) external onlyOwner {
        _addTrackedProxy(offerTokenProxies, proxy);
        emit OfferTokenProxyRegistered(proxy);
    }

    function removeOfferTokenProxy(address proxy) external onlyOwner {
        _removeTrackedProxy(offerTokenProxies, proxy);
        emit OfferTokenProxyRemoved(proxy);
    }

    function getOfferTokenProxies() external view returns (address[] memory) {
        return offerTokenProxies;
    }

    // Offer Token Sale ----------------------------------------------------

    function setOfferTokenSaleImplementation(address newImplementation) external onlyOwner {
        _validateNewAddress(offerTokenSaleImplementation, newImplementation);
        offerTokenSaleImplementation = newImplementation;
        emit OfferTokenSaleImplementationUpdated(newImplementation);
    }

    function setOfferTokenSaleBeacon(address newBeacon) external onlyOwner {
        _validateNewAddress(offerTokenSaleBeacon, newBeacon);
        offerTokenSaleBeacon = newBeacon;
        emit OfferTokenSaleBeaconUpdated(newBeacon);
    }

    function setOfferTokenSaleFactory(address newFactory) external onlyOwner {
        _validateNewAddress(offerTokenSaleFactory, newFactory);
        offerTokenSaleFactory = newFactory;
        emit OfferTokenSaleFactoryUpdated(newFactory);
    }

    function addOfferTokenSaleProxy(address proxy) external onlyOwner {
        _addTrackedProxy(offerTokenSaleProxies, proxy);
        emit OfferTokenSaleProxyRegistered(proxy);
    }

    function removeOfferTokenSaleProxy(address proxy) external onlyOwner {
        _removeTrackedProxy(offerTokenSaleProxies, proxy);
        emit OfferTokenSaleProxyRemoved(proxy);
    }

    function getOfferTokenSaleProxies() external view returns (address[] memory) {
        return offerTokenSaleProxies;
    }

    // Token Distributor ---------------------------------------------------

    function setTokenDistributorImplementation(address newImplementation) external onlyOwner {
        _validateNewAddress(tokenDistributorImplementation, newImplementation);
        tokenDistributorImplementation = newImplementation;
        emit TokenDistributorImplementationUpdated(newImplementation);
    }

    function setTokenDistributorBeacon(address newBeacon) external onlyOwner {
        _validateNewAddress(tokenDistributorBeacon, newBeacon);
        tokenDistributorBeacon = newBeacon;
        emit TokenDistributorBeaconUpdated(newBeacon);
    }

    function setTokenDistributorFactory(address newFactory) external onlyOwner {
        _validateNewAddress(tokenDistributorFactory, newFactory);
        tokenDistributorFactory = newFactory;
        emit TokenDistributorFactoryUpdated(newFactory);
    }

    function addTokenDistributorProxy(address proxy) external onlyOwner {
        _addTrackedProxy(tokenDistributorProxies, proxy);
        emit TokenDistributorProxyRegistered(proxy);
    }

    function removeTokenDistributorProxy(address proxy) external onlyOwner {
        _removeTrackedProxy(tokenDistributorProxies, proxy);
        emit TokenDistributorProxyRemoved(proxy);
    }

    function getTokenDistributorProxies() external view returns (address[] memory) {
        return tokenDistributorProxies;
    }

    // Token Distributor By Holdings --------------------------------------

    function setTokenDistributorByHoldingsImplementation(address newImplementation) external onlyOwner {
        _validateNewAddress(tokenDistributorByHoldingsImplementation, newImplementation);
        tokenDistributorByHoldingsImplementation = newImplementation;
        emit TokenDistributorByHoldingsImplementationUpdated(newImplementation);
    }

    function setTokenDistributorByHoldingsBeacon(address newBeacon) external onlyOwner {
        _validateNewAddress(tokenDistributorByHoldingsBeacon, newBeacon);
        tokenDistributorByHoldingsBeacon = newBeacon;
        emit TokenDistributorByHoldingsBeaconUpdated(newBeacon);
    }

    function setTokenDistributorByHoldingsFactory(address newFactory) external onlyOwner {
        _validateNewAddress(tokenDistributorByHoldingsFactory, newFactory);
        tokenDistributorByHoldingsFactory = newFactory;
        emit TokenDistributorByHoldingsFactoryUpdated(newFactory);
    }

    function addTokenDistributorByHoldingsProxy(address proxy) external onlyOwner {
        _addTrackedProxy(tokenDistributorByHoldingsProxies, proxy);
        emit TokenDistributorByHoldingsProxyRegistered(proxy);
    }

    function removeTokenDistributorByHoldingsProxy(address proxy) external onlyOwner {
        _removeTrackedProxy(tokenDistributorByHoldingsProxies, proxy);
        emit TokenDistributorByHoldingsProxyRemoved(proxy);
    }

    function getTokenDistributorByHoldingsProxies() external view returns (address[] memory) {
        return tokenDistributorByHoldingsProxies;
    }

    // Offer Sale Allowlist ------------------------------------------------

    function setOfferSaleAllowlist(address newAllowlist) external onlyOwner {
        _validateNewAddress(offerSaleAllowlist, newAllowlist);
        offerSaleAllowlist = newAllowlist;
        emit OfferSaleAllowlistUpdated(newAllowlist);
    }

    // Internal helpers ----------------------------------------------------

    function _validateNewAddress(address current, address candidate) private pure {
        if (candidate == address(0)) {
            revert ZeroAddress();
        }
        if (candidate == current) {
            revert UnchangedAddress();
        }
    }

    function _addTrackedProxy(address[] storage list, address proxy) private {
        if (proxy == address(0)) {
            revert ZeroAddress();
        }
        uint256 length = list.length;
        for (uint256 i; i < length; ++i) {
            if (list[i] == proxy) {
                revert AddressAlreadyTracked();
            }
        }
        list.push(proxy);
    }

    function _removeTrackedProxy(address[] storage list, address proxy) private {
        if (proxy == address(0)) {
            revert ZeroAddress();
        }
        uint256 length = list.length;
        for (uint256 i; i < length; ++i) {
            if (list[i] == proxy) {
                list[i] = list[length - 1];
                list.pop();
                return;
            }
        }
        revert AddressNotTracked();
    }
}
