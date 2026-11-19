export const CHAIN_ID = {
  STELLAR_TESTNET: 'stellar-testnet',
  STELLAR_MAINNET: 'stellar-mainnet',
  BASE_TESTNET: 'base-testnet',
  BASE_MAINNET: 'base-mainnet',
} as const

export type ChainId = (typeof CHAIN_ID)[keyof typeof CHAIN_ID]
