use strum_macros::{Display, EnumString};

#[derive(Debug, Display, PartialEq, EnumString)]
pub enum Methods {
    #[strum(serialize = "tlock_ping")]
    TlockPing,
    #[strum(serialize = "plugin_version")]
    PluginVersion,
    #[strum(serialize = "plugin_name")]
    PluginName,

    // Keyring Namespace
    #[strum(serialize = "keyring_createAccount")]
    KeyringCreateAccount,
    #[strum(serialize = "keyring_deleteAccount")]
    KeyringDeleteAccount,
    #[strum(serialize = "eth_accounts")]
    EthAccounts,
    #[strum(serialize = "personal_sign")]
    PersonalSign,
    //? Compatibility alias for MetaMask-style method
    #[strum(serialize = "eth_signTypedData", serialize = "eth_signTypedData_v4")]
    EthSignTypedData,
    #[strum(serialize = "eth_sendRawTransaction")]
    EthSendRawTransaction,

    // Ethereum JSON-RPC Api
    // https://ethereum.org/en/developers/docs/apis/json-rpc/#json-rpc-methods

    // Web3 Namespace
    #[strum(serialize = "eth_sendTransaction")]
    EthSendTransaction,
    #[strum(serialize = "eth_getBalance")]
    EthGetBalance,
    #[strum(serialize = "web3_clientVersion")]
    Web3ClientVersion,
    #[strum(serialize = "web3_sha3")]
    Web3Sha3,

    // Net Namespace
    #[strum(serialize = "net_version")]
    NetVersion,
    #[strum(serialize = "net_listening")]
    NetListening,
    #[strum(serialize = "net_peerCount")]
    NetPeerCount,

    // Eth Namespace
    #[strum(serialize = "eth_blockNumber")]
    EthBlockNumber,
    #[strum(serialize = "eth_call")]
    EthCall,
    #[strum(serialize = "eth_chainId")]
    EthChainId,
    #[strum(serialize = "eth_coinbase")]
    EthCoinbase,
    #[strum(serialize = "eth_estimateGas")]
    EthEstimateGas,
    #[strum(serialize = "eth_feeHistory")]
    EthFeeHistory,
    #[strum(serialize = "eth_gasPrice")]
    EthGasPrice,
    #[strum(serialize = "eth_getBlockByHash")]
    EthGetBlockByHash,
    #[strum(serialize = "eth_getBlockByNumber")]
    EthGetBlockByNumber,
    #[strum(serialize = "eth_getBlockTransactionCountByHash")]
    EthGetBlockTransactionCountByHash,
    #[strum(serialize = "eth_getBlockTransactionCountByNumber")]
    EthGetBlockTransactionCountByNumber,
    #[strum(serialize = "eth_getCode")]
    EthGetCode,
    #[strum(serialize = "eth_getFilterChanges")]
    EthGetFilterChanges,
    #[strum(serialize = "eth_getFilterLogs")]
    EthGetFilterLogs,
    #[strum(serialize = "eth_getLogs")]
    EthGetLogs,
    #[strum(serialize = "eth_getProof")]
    EthGetProof,
    #[strum(serialize = "eth_getStorageAt")]
    EthGetStorageAt,
    #[strum(serialize = "eth_getTransactionByBlockHashAndIndex")]
    EthGetTransactionByBlockHashAndIndex,
    #[strum(serialize = "eth_getTransactionByHash")]
    EthGetTransactionByHash,
    #[strum(serialize = "eth_getTransactionCount")]
    EthGetTransactionCount,
    #[strum(serialize = "eth_getTransactionReceipt")]
    EthGetTransactionReceipt,
    #[strum(serialize = "eth_getUncleCountByBlockHash")]
    EthGetUncleCountByBlockHash,
    #[strum(serialize = "eth_getUncleCountByBlockNumber")]
    EthGetUncleCountByBlockNumber,
    #[strum(serialize = "eth_newBlockFilter")]
    EthNewBlockFilter,
    #[strum(serialize = "eth_newFilter")]
    EthNewFilter,
    #[strum(serialize = "eth_newPendingTransactionFilter")]
    EthNewPendingTransactionFilter,
    // TODO: This is a Metamask method, not officially part of eth json-rpc. Should it be included?
    // #[strum(serialize = "eth_requestAccounts")]
    // EthRequestAccounts,
    #[strum(serialize = "eth_subscribe")]
    EthSubscribe,
    #[strum(serialize = "eth_syncing")]
    EthSyncing,
    #[strum(serialize = "eth_uninstallFilter")]
    EthUninstallFilter,
    #[strum(serialize = "eth_unsubscribe")]
    EthUnsubscribe,

    // Wallet Namespace
    #[strum(serialize = "wallet_addEthereumChain")]
    WalletAddEthereumChain,
    #[strum(serialize = "wallet_getCallStatus")]
    WalletGetCallStatus,
    #[strum(serialize = "wallet_getCapabilities")]
    WalletGetCapabilities,
    #[strum(serialize = "wallet_getPermissions")]
    WalletGetPermissions,
    #[strum(serialize = "wallet_registerOnboarding")]
    WalletRegisterOnboarding,
    #[strum(serialize = "wallet_requestPermissions")]
    WalletRequestPermissions,
    #[strum(serialize = "wallet_revokePermissions")]
    WalletRevokePermissions,
    #[strum(serialize = "wallet_scanQRCode")]
    WalletScanQrCode,
    #[strum(serialize = "wallet_sendCalls")]
    WalletSendCalls,
    #[strum(serialize = "wallet_switchEthereumChain")]
    WalletSwitchEthereumChain,
    #[strum(serialize = "wallet_watchAsset")]
    WalletWatchAsset,
}
