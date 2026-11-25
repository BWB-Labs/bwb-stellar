import { Keypair, StrKey } from '@stellar/stellar-sdk/base'

export type HashSigner = {
  address: string
  sign(hash: Uint8Array): Promise<Uint8Array>
}

export type AddressSignature = {
  address: string
  signature: string
}

export type PrivyWallets = {
  rawSign(
    walletId: string,
    input: { params: { hash: string } },
  ): Promise<{ signature: string }>
}

const HEX_SIGNATURE = /^0x[0-9a-fA-F]{128}$/

export function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

export function fromHex(hex: string): Uint8Array {
  return Uint8Array.from(hex.replace(/^0x/, '').match(/../g) ?? [], (byte) => parseInt(byte, 16))
}

export function keypairSigner(keypair: Keypair): HashSigner {
  return {
    address: keypair.publicKey(),
    async sign(hash) {
      return Uint8Array.from(keypair.sign(hash))
    },
  }
}

export function privySigner(
  wallets: PrivyWallets,
  { walletId, address }: { walletId: string; address: string },
): HashSigner {
  return {
    address,
    async sign(hash) {
      const { signature } = await wallets.rawSign(walletId, {
        params: { hash: `0x${toHex(hash)}` },
      })
      if (!HEX_SIGNATURE.test(signature)) {
        throw new Error(`Privy returned no ed25519 signature for ${address}`)
      }
      return fromHex(signature)
    },
  }
}

export function isSignatureOf(
  address: string,
  hash: string,
  signature: string,
): boolean {
  if (!StrKey.isValidEd25519PublicKey(address)) return false
  if (!HEX_SIGNATURE.test(signature)) return false
  try {
    return Keypair.fromPublicKey(address).verify(
      fromHex(hash),
      fromHex(signature),
    )
  } catch {
    return false
  }
}
