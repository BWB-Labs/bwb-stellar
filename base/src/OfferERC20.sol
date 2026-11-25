// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {ERC20Upgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol";
import {
    ERC20PausableUpgradeable
} from "@openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PausableUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {Arrays} from "@openzeppelin/contracts/utils/Arrays.sol";

/**
 * @title OfferERC20
 * @notice ERC20 token representing a private offer with an updatable metadata URI.
 * @custom:oz-upgrades-unsafe-allow constructor
 */
contract OfferERC20 is Initializable, ERC20Upgradeable, ERC20PausableUpgradeable, OwnableUpgradeable {
    using Arrays for uint256[];

    string private _offerURI;
    mapping(address => bool) private transferWhitelist;

    address private _transferAdmin;

    struct Snapshots {
        uint256[] ids;
        uint256[] values;
    }

    mapping(address => Snapshots) private _accountBalanceSnapshots;
    Snapshots private _totalSupplySnapshots;
    uint256 private _currentSnapshotId;

    /// @notice Emitted when the offer URI is updated
    event OfferURIUpdated(string newURI);
    /// @notice Emitted when an address is toggled in the transfer whitelist
    event TransferWhitelistUpdated(address indexed account, bool allowed);
    /// @notice Emitted whenever a snapshot is taken
    event Snapshot(uint256 id);
    /// @notice Emitted when the transfer admin forcibly moves tokens
    event AdminTransfer(address indexed admin, address indexed from, address indexed to, uint256 amount);

    error NotTransferAdmin(address account);
    error InvalidAdminTransferEndpoint(address from, address to);

    constructor() {
        _disableInitializers();
    }

    function initialize(
        string memory name_,
        string memory symbol_,
        address initialOwner,
        uint256 initialSupply,
        string memory offerURI_,
        address transferAdmin_,
        address[] calldata initialWhitelistedAccounts
    ) external initializer {
        require(initialOwner != address(0), "Owner required");
        require(transferAdmin_ != address(0), "Transfer admin required");

        __ERC20_init(name_, symbol_);
        __Ownable_init(initialOwner);
        __ERC20Pausable_init();

        _offerURI = offerURI_;
        _mint(initialOwner, initialSupply);
        _transferAdmin = transferAdmin_;

        transferWhitelist[initialOwner] = true;
        emit TransferWhitelistUpdated(initialOwner, true);

        uint256 length = initialWhitelistedAccounts.length;
        for (uint256 i = 0; i < length; ++i) {
            address account = initialWhitelistedAccounts[i];
            require(account != address(0), "Whitelisted account required");
            if (!transferWhitelist[account]) {
                transferWhitelist[account] = true;
                emit TransferWhitelistUpdated(account, true);
            }
        }
    }

    /// @notice Halt all token transfers
    function pause() external onlyOwner {
        _pause();
    }

    /// @notice Resume token transfers
    function unpause() external onlyOwner {
        _unpause();
    }

    /// @notice Tokens are non-divisible and always mint as whole units
    function decimals() public pure override returns (uint8) {
        return 0;
    }

    /// @notice Address authorized to perform forced transfers across all tokens on this implementation
    function transferAdmin() public view returns (address) {
        return _transferAdmin;
    }

    /// @notice Admin-only hook to forcibly transfer tokens between any two addresses
    function adminTransfer(address from, address to, uint256 amount) external onlyTransferAdmin {
        if (from == address(0) || to == address(0)) {
            revert InvalidAdminTransferEndpoint(from, to);
        }
        _executeUpdate(from, to, amount, false);
        emit AdminTransfer(_transferAdmin, from, to, amount);
    }

    /// @notice Take a point-in-time snapshot of balances and total supply
    function snapshot() external onlyOwner returns (uint256) {
        return _snapshot();
    }

    /// @notice Retrieve the balance of an account for a specific snapshot id
    function balanceOfAt(address account, uint256 snapshotId) external view returns (uint256) {
        (bool snapshotted, uint256 value) = _valueAt(snapshotId, _accountBalanceSnapshots[account]);
        return snapshotted ? value : balanceOf(account);
    }

    /// @notice Retrieve the total supply for a specific snapshot id
    function totalSupplyAt(uint256 snapshotId) external view returns (uint256) {
        (bool snapshotted, uint256 value) = _valueAt(snapshotId, _totalSupplySnapshots);
        return snapshotted ? value : totalSupply();
    }

    /// @notice Return the current metadata URI describing the offer
    function offerURI() external view returns (string memory) {
        return _offerURI;
    }

    /**
     * @notice Returns true when an address is authorized as a transfer participant (sender or recipient).
     */
    function isTransferWhitelisted(address account) external view returns (bool) {
        return transferWhitelist[account];
    }

    /**
     * @notice Update the offer metadata URI
     * @param newURI Metadata URI (e.g., IPFS/HTTPS) describing the offer
     */
    function updateOfferURI(string calldata newURI) external onlyOwner {
        _offerURI = newURI;
        emit OfferURIUpdated(newURI);
    }

    /**
     * @notice Allow or block an address from participating in secondary transfers.
     *         Whitelisted addresses may send tokens to anyone; non-whitelisted holders
     *         may only transfer tokens back to whitelisted endpoints (e.g., issuer contracts).
     */
    function setTransferWhitelist(address account, bool allowed) external onlyOwner {
        require(account != address(0), "Account required");
        transferWhitelist[account] = allowed;
        emit TransferWhitelistUpdated(account, allowed);
    }

    function version() external pure returns (string memory) {
        return "1.0.0";
    }

    function _update(address from, address to, uint256 value)
        internal
        override(ERC20Upgradeable, ERC20PausableUpgradeable)
    {
        if (from != address(0) && to != address(0)) {
            bool fromWhitelisted = transferWhitelist[from];
            bool toWhitelisted = transferWhitelist[to];
            require(fromWhitelisted || toWhitelisted, "Transfer restricted");
        }

        _executeUpdate(from, to, value, true);
    }

    function _executeUpdate(address from, address to, uint256 value, bool enforcePause) private {
        _beforeSnapshotUpdate(from, to);
        if (enforcePause) {
            ERC20PausableUpgradeable._update(from, to, value);
        } else {
            ERC20Upgradeable._update(from, to, value);
        }
    }

    function _snapshot() internal returns (uint256) {
        unchecked {
            _currentSnapshotId += 1;
        }
        emit Snapshot(_currentSnapshotId);
        return _currentSnapshotId;
    }

    function _beforeSnapshotUpdate(address from, address to) private {
        if (_currentSnapshotId == 0) {
            return;
        }
        if (from == address(0)) {
            _updateAccountSnapshot(to);
            _updateTotalSupplySnapshot();
        } else if (to == address(0)) {
            _updateAccountSnapshot(from);
            _updateTotalSupplySnapshot();
        } else {
            _updateAccountSnapshot(from);
            _updateAccountSnapshot(to);
        }
    }

    function _valueAt(uint256 snapshotId, Snapshots storage snapshots) private view returns (bool, uint256) {
        require(snapshotId > 0, "Snapshot: id is 0");
        require(snapshotId <= _currentSnapshotId, "Snapshot: nonexistent id");

        uint256 index = snapshots.ids.findUpperBound(snapshotId);

        if (index == snapshots.ids.length) {
            return (false, 0);
        }
        return (true, snapshots.values[index]);
    }

    function _updateAccountSnapshot(address account) private {
        _updateSnapshot(_accountBalanceSnapshots[account], balanceOf(account));
    }

    function _updateTotalSupplySnapshot() private {
        _updateSnapshot(_totalSupplySnapshots, totalSupply());
    }

    function _updateSnapshot(Snapshots storage snapshots, uint256 currentValue) private {
        uint256 currentId = _currentSnapshotId;
        if (currentId != 0 && _lastSnapshotId(snapshots.ids) < currentId) {
            snapshots.ids.push(currentId);
            snapshots.values.push(currentValue);
        }
    }

    function _lastSnapshotId(uint256[] storage ids) private view returns (uint256) {
        if (ids.length == 0) {
            return 0;
        }
        return ids[ids.length - 1];
    }

    modifier onlyTransferAdmin() {
        _onlyTransferAdmin();
        _;
    }

    function _onlyTransferAdmin() internal view {
        if (_msgSender() != _transferAdmin) {
            revert NotTransferAdmin(_msgSender());
        }
    }

    /// @dev shrink the gap when adding next updates
    /// Do not add variables after the gap.
    /// Do not reorder existing ones.
    uint256[50] private __gap;
}
