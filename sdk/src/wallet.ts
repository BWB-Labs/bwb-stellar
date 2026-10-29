import { type Asset, Operation, StrKey, type xdr } from '@stellar/stellar-sdk/base'

import { refuse, succeed, type Result } from './result.js'

export type PrivyLinkedAccount = {
  type: string
  chain_type?: string
  address?: string
  wallet_client_type?: string
  connector_type?: string
}

export type PrivyUser = { linked_accounts: Array<PrivyLinkedAccount> }

export type PrivyUsers = {
  _get(userId: string): Promise<PrivyUser>
  pregenerateWallets(
    userId: string,
    input: { wallets: Array<{ chain_type: 'stellar' }> },
  ): Promise<PrivyUser>
}

export const WALLET_FAILURE = {
  PROVISIONING_REFUSED: 'provisioningRefused',
} as const

export type WalletFailure = {
  code: (typeof WALLET_FAILURE)[keyof typeof WALLET_FAILURE]
}

function embeddedStellarAddress({ linked_accounts }: PrivyUser): string | undefined {
  return linked_accounts.find(
    (account) =>
      account.type === 'wallet' &&
      account.wallet_client_type === 'privy' &&
      account.connector_type === 'embedded' &&
      account.chain_type === 'stellar',
  )?.address
}

export async function ensureStellarWallet(
  users: PrivyUsers,
  privyUserId: string,
): Promise<Result<{ address: string }, WalletFailure>> {
  const refused = (message: string) =>
    refuse({ code: WALLET_FAILURE.PROVISIONING_REFUSED }, message)
  try {
    const address =
      embeddedStellarAddress(await users._get(privyUserId)) ??
      embeddedStellarAddress(
        await users.pregenerateWallets(privyUserId, {
          wallets: [{ chain_type: 'stellar' }],
        }),
      )
    if (address === undefined || !StrKey.isValidEd25519PublicKey(address)) {
      return refused(`Privy returned no Stellar wallet for ${privyUserId}`)
    }
    return succeed({ address }, 'Stellar wallet provisioned')
  } catch (error) {
    return refused(`Privy created no Stellar wallet: ${String(error)}`)
  }
}

export type WalletOpening = {
  address: string
  sponsor: string
  asset: Asset
  opensAccount: boolean
}

export function walletOpeningOperations({
  address,
  sponsor,
  asset,
  opensAccount,
}: WalletOpening): Array<xdr.Operation> {
  return [
    Operation.beginSponsoringFutureReserves({ sponsoredId: address, source: sponsor }),
    ...(opensAccount
      ? [Operation.createAccount({ destination: address, startingBalance: '0', source: sponsor })]
      : []),
    Operation.changeTrust({ asset, source: address }),
    Operation.endSponsoringFutureReserves({ source: address }),
  ]
}
