import { Asset, hash, Keypair, Networks, xdr } from '@stellar/stellar-sdk/base'
import { Server } from '@stellar/stellar-sdk/rpc'

export const NETWORK = {
  TESTNET: 'testnet',
  MAINNET: 'mainnet',
} as const

export type Network = (typeof NETWORK)[keyof typeof NETWORK]

const PASSPHRASE = {
  [NETWORK.TESTNET]: Networks.TESTNET,
  [NETWORK.MAINNET]: Networks.PUBLIC,
} satisfies Record<Network, string>

export function passphraseOf(network: Network): string {
  return PASSPHRASE[network]
}

const CIRCLE_USDC = {
  [NETWORK.TESTNET]: {
    issuer: 'GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5',
    contract: 'CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA',
  },
  [NETWORK.MAINNET]: {
    issuer: 'GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN',
    contract: 'CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75',
  },
} satisfies Record<Network, { issuer: string; contract: string }>

export function usdcOf(network: Network): { asset: Asset; contract: string } {
  const { issuer, contract } = CIRCLE_USDC[network]
  return { asset: new Asset('USDC', issuer), contract }
}

export function networkIdOf(network: Network): Uint8Array {
  return hash(Buffer.from(passphraseOf(network)))
}

export class StellarChainMismatchError extends Error {}

export type StellarRpc = <T>(
  call: (server: Server, passphrase: string) => Promise<T>,
) => Promise<T>

export type RpcSettings = {
  network: Network
  rpcUrls: Array<string>
}

export function connect({ network, rpcUrls }: RpcSettings): StellarRpc {
  if (rpcUrls.length === 0) {
    throw new Error('No Stellar RPC URL given')
  }
  const expected = passphraseOf(network)
  return async (call) => {
    for (const rpcUrl of rpcUrls) {
      const server = new Server(rpcUrl)
      let passphrase: string
      try {
        ;({ passphrase } = await server.getNetwork())
      } catch {
        continue
      }
      if (passphrase !== expected) {
        throw new StellarChainMismatchError(
          `Stellar RPC endpoint ${new URL(rpcUrl).host} serves "${passphrase}", expected "${expected}"`,
        )
      }
      return call(server, passphrase)
    }
    throw new Error(
      `No Stellar RPC endpoint answered (${rpcUrls.map((rpcUrl) => new URL(rpcUrl).host).join(', ')})`,
    )
  }
}

export function accountKey(address: string): xdr.LedgerKey {
  return xdr.LedgerKey.account(
    new xdr.LedgerKeyAccount({
      accountId: Keypair.fromPublicKey(address).xdrPublicKey(),
    }),
  )
}
