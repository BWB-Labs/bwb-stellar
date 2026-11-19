import {
  Address,
  authorizeEntry,
  buildAuthorizationEntryPreimage,
  hash,
  inspectAuthEntry,
  Operation,
  scValToNative,
  StrKey,
  xdr,
} from '@stellar/stellar-sdk/base'
import { Api, type Server } from '@stellar/stellar-sdk/rpc'

import { refuse, succeed, type Result, type ResultError } from '../result.js'
import {
  SALE_REFUSAL,
  SALE_STATE,
  type AuthorizedPurchase,
  type PurchaseReceipt,
  type Sale,
  type SaleRefusal,
  type SaleRefusalCode,
  type SaleState,
  type SaleView,
} from '../sale.js'
import { passphraseOf, STELLAR_NETWORK, StellarChainMismatchError, type StellarChainId, type StellarRpc } from './network.js'
import { fromHex, isSignatureOf, toHex } from './signing.js'
import { scv, simulateOperation, simulateRead, SorobanSimulationError } from './soroban.js'
import { readSignerSet } from './treasury.js'

const SIMULATION_SOURCE = StrKey.encodeEd25519PublicKey(new Uint8Array(32))
const ENTRY_LIFETIME_LEDGERS = 360
const PURCHASED_EVENT = 'purchased'

const CONTRACT_REFUSALS: Record<string, SaleRefusalCode> = {
  10: SALE_REFUSAL.INSUFFICIENT_BALANCE,
  11: SALE_REFUSAL.BALANCE_FROZEN,
  13: SALE_REFUSAL.TRUSTLINE_MISSING,
  6100: SALE_REFUSAL.INVESTOR_NOT_ENABLED,
  6101: SALE_REFUSAL.ALLOCATION_EXCEEDED,
  6107: SALE_REFUSAL.TOO_MANY_RECEIPTS,
  6201: SALE_REFUSAL.SALE_NOT_ACTIVE,
  6206: SALE_REFUSAL.NOT_WHOLE_QUOTAS,
  6207: SALE_REFUSAL.EXCEEDS_SUPPLY,
  6220: SALE_REFUSAL.TOO_MANY_PURCHASES,
  6228: SALE_REFUSAL.BELOW_MINIMUM,
}

const SALE_STATES: Record<string, SaleState> = {
  Preparing: SALE_STATE.PREPARING,
  Active: SALE_STATE.ACTIVE,
  Closing: SALE_STATE.CLOSING,
  Successful: SALE_STATE.SUCCESSFUL,
  Failed: SALE_STATE.FAILED,
}

function refused(code: SaleRefusalCode, message: string): ResultError<SaleRefusal> {
  return refuse({ code }, message)
}

function contractRefusal(simulationError: string): ResultError<SaleRefusal> {
  const [, contractCode] = simulationError.match(/Error\(Contract, #(\d+)\)/) ?? []
  const code = contractCode === undefined ? undefined : CONTRACT_REFUSALS[contractCode]
  return refused(code ?? SALE_REFUSAL.CONTRACT_REFUSED, `The sale refused the call: ${simulationError}`)
}

function contractCallOf(invocation: xdr.SorobanAuthorizedInvocation) {
  const authorized = invocation.function
  if (authorized.type !== 'sorobanAuthorizedFunctionTypeContractFn') return undefined
  const { contractAddress, functionName, args } = authorized.contractFn
  return {
    contract: Address.fromScAddress(contractAddress).toString(),
    method: functionName.toString(),
    args: args.map((arg) => scValToNative(arg)),
  }
}

function coversPurchase(
  { rootInvocation }: xdr.SorobanAuthorizationEntry,
  { sale, buyer, amount }: { sale: string; buyer: string; amount: bigint },
): boolean {
  const root = contractCallOf(rootInvocation)
  const [payment, ...others] = rootInvocation.subInvocations
  const transfer = payment === undefined ? undefined : contractCallOf(payment)
  return (
    others.length === 0 &&
    root?.contract === sale &&
    root.method === 'buy' &&
    root.args[0] === buyer &&
    root.args[1] === amount &&
    transfer?.method === 'transfer' &&
    transfer.args[0] === buyer &&
    transfer.args[1] === sale &&
    transfer.args[2] === amount
  )
}

function coversClose({ rootInvocation }: xdr.SorobanAuthorizationEntry, { sale, treasury }: { sale: string; treasury: string }) {
  const root = contractCallOf(rootInvocation)
  return (
    rootInvocation.subInvocations.length === 0 &&
    root?.contract === sale &&
    root.method === 'close' &&
    root.args.length === 1 &&
    root.args[0] === treasury
  )
}

function eventsOf(resultMeta: string): Array<xdr.ContractEvent> | undefined {
  try {
    const meta = xdr.TransactionMeta.fromXDR(resultMeta, 'base64')
    return meta.type === 'v4' ? meta.v4.operations.flatMap(({ events }) => events) : undefined
  } catch {
    return undefined
  }
}

function purchaseIn(event: xdr.ContractEvent, sale: string, buyer: string): PurchaseReceipt | undefined {
  if (event.contractId === null) return undefined
  if (Address.fromScAddress(xdr.ScAddress.scAddressTypeContract(event.contractId)).toString() !== sale) return undefined
  const { topics, data } = event.body.v0
  const [name, investor] = topics.map((topic) => scValToNative(topic))
  if (name !== PURCHASED_EVENT || investor !== buyer) return undefined
  const { tokens, paid } = scValToNative(data)
  return typeof tokens === 'bigint' && typeof paid === 'bigint' ? { quotas: tokens, paid } : undefined
}

export function stellarSale(chainId: StellarChainId, rpc: StellarRpc, address: string): Sale {
  const passphrase = passphraseOf(STELLAR_NETWORK[chainId])

  function simulate(server: Server, operation: xdr.Operation) {
    return simulateOperation(server, passphrase, { source: SIMULATION_SOURCE, operation })
  }

  function entryHashOf(entry: xdr.SorobanAuthorizationEntry, expirationLedger: number) {
    return toHex(hash(buildAuthorizationEntryPreimage(entry, expirationLedger, passphrase).toXDR()))
  }

  async function signableEntry(
    server: Server,
    operation: xdr.Operation,
    { signer, covers }: { signer: string; covers: (entry: xdr.SorobanAuthorizationEntry) => boolean },
  ): Promise<Result<{ entry: string; entryHash: string; expirationLedger: number }, SaleRefusal>> {
    const simulation = await simulate(server, operation)
    if (!Api.isSimulationSuccess(simulation)) return contractRefusal(simulation.error)
    const entries = simulation.result?.auth ?? []
    const [entry] = entries
    if (entry === undefined || entries.length !== 1 || inspectAuthEntry(entry).address !== signer || !covers(entry)) {
      return refused(SALE_REFUSAL.UNEXPECTED_AUTHORIZATION, `The call did not ask ${signer} for one entry covering it`)
    }
    const expirationLedger = simulation.latestLedger + ENTRY_LIFETIME_LEDGERS
    return succeed(
      {
        entry: entry.toXDR('base64'),
        entryHash: entryHashOf(entry, expirationLedger),
        expirationLedger,
      },
      `Entry prepared for ${signer} to sign`,
    )
  }

  async function authorized(
    server: Server,
    operation: xdr.Operation,
    expirationLedger: number,
  ): Promise<Result<AuthorizedPurchase, SaleRefusal>> {
    const simulation = await simulate(server, operation)
    if (simulation.latestLedger >= expirationLedger) {
      return refused(SALE_REFUSAL.ENTRY_EXPIRED, `The entry expired at ledger ${expirationLedger}`)
    }
    if (!Api.isSimulationSuccess(simulation)) return contractRefusal(simulation.error)
    return succeed({ chainId, invocation: operation.toXDR('base64') }, 'Signed call authorized')
  }

  function closeEntryOf(entry: string, treasury: string): xdr.SorobanAuthorizationEntry | undefined {
    try {
      const unsigned = xdr.SorobanAuthorizationEntry.fromXDR(entry, 'base64')
      return inspectAuthEntry(unsigned).address === treasury && coversClose(unsigned, { sale: address, treasury })
        ? unsigned
        : undefined
    } catch {
      return undefined
    }
  }

  function closeOperation(treasury: string, auth: Array<xdr.SorobanAuthorizationEntry>) {
    return Operation.invokeContractFunction({ contract: address, function: 'close', args: [scv.address(treasury)], auth })
  }

  function buyOperation(buyer: string, amount: bigint, auth: Array<xdr.SorobanAuthorizationEntry>) {
    return Operation.invokeContractFunction({
      contract: address,
      function: 'buy',
      args: [scv.address(buyer), scv.i128(amount)],
      auth,
    })
  }

  async function onChain<T>(call: (server: Server) => Promise<Result<T, SaleRefusal>>): Promise<Result<T, SaleRefusal>> {
    try {
      return await rpc((server, served) =>
        toHex(hash(served)) === chainId
          ? call(server)
          : Promise.resolve(refused(SALE_REFUSAL.CHAIN_MISMATCH, `The client serves network "${served}", not chain ${chainId}`)),
      )
    } catch (error) {
      if (error instanceof SorobanSimulationError) return contractRefusal(error.message)
      if (error instanceof StellarChainMismatchError) return refused(SALE_REFUSAL.CHAIN_MISMATCH, error.message)
      return refused(SALE_REFUSAL.CHAIN_UNAVAILABLE, `The chain did not answer: ${String(error)}`)
    }
  }

  return {
    buy: async ({ buyer, amount }) => {
      if (!StrKey.isValidEd25519PublicKey(buyer)) {
        return refused(SALE_REFUSAL.INVALID_BUYER, `${buyer} is not an account address`)
      }
      return onChain(async (server) => {
        const signable = await signableEntry(server, buyOperation(buyer, amount, []), {
          signer: buyer,
          covers: (entry) => coversPurchase(entry, { sale: address, buyer, amount }),
        })
        return signable.success ? succeed({ chainId, buyer, amount, ...signable.data }, signable.message) : signable
      })
    },

    async authorize({ prepared: { buyer, amount, entry, expirationLedger }, signature }) {
      let signed: xdr.SorobanAuthorizationEntry
      try {
        signed = await authorizeEntry(
          xdr.SorobanAuthorizationEntry.fromXDR(entry, 'base64'),
          async () => ({ signature: fromHex(signature), publicKey: buyer }),
          expirationLedger,
          passphrase,
          buyer,
        )
      } catch (error) {
        return refused(SALE_REFUSAL.SIGNATURE_MISMATCH, `The signature is not of the entry: ${String(error)}`)
      }
      return onChain((server) => authorized(server, buyOperation(buyer, amount, [signed]), expirationLedger))
    },

    close: async ({ treasury }) => {
      if (!StrKey.isValidEd25519PublicKey(treasury)) {
        return refused(SALE_REFUSAL.INVALID_TREASURY, `${treasury} is not an account address`)
      }
      return onChain(async (server) => {
        const signable = await signableEntry(server, closeOperation(treasury, []), {
          signer: treasury,
          covers: (entry) => coversClose(entry, { sale: address, treasury }),
        })
        return signable.success ? succeed({ chainId, treasury, ...signable.data }, signable.message) : signable
      })
    },

    async authorizeClose({ prepared: { treasury, entry, expirationLedger }, signatures }) {
      const unsigned = closeEntryOf(entry, treasury)
      if (unsigned === undefined) {
        return refused(SALE_REFUSAL.UNEXPECTED_AUTHORIZATION, `The entry is not ${treasury}'s close of ${address}`)
      }
      const byOfficer = new Map(signatures.map(({ address: signer, signature }) => [signer, signature]))
      let signed: xdr.SorobanAuthorizationEntry
      try {
        const entryHash = entryHashOf(unsigned, expirationLedger)
        const forged = [...byOfficer].find(([signer, signature]) => !isSignatureOf(signer, entryHash, signature))
        if (forged !== undefined) {
          return refused(SALE_REFUSAL.SIGNATURE_MISMATCH, `The signature of ${forged[0]} is not of the entry`)
        }
        const sorted = [...byOfficer]
          .map(([signer, signature]) => {
            const publicKey = StrKey.decodeEd25519PublicKey(signer)
            return { publicKey, signature: fromHex(signature), order: toHex(publicKey) }
          })
          .sort((a, b) => (a.order < b.order ? -1 : 1))
        signed = await authorizeEntry(
          unsigned,
          async () => ({
            signatureScVal: scv.vec(
              sorted.map(({ publicKey, signature }) =>
                scv.struct({ public_key: xdr.ScVal.scvBytes(publicKey), signature: xdr.ScVal.scvBytes(signature) }),
              ),
            ),
          }),
          expirationLedger,
          passphrase,
          treasury,
        )
      } catch (error) {
        return refused(SALE_REFUSAL.SIGNATURE_MISMATCH, `The signatures cannot sign the entry: ${String(error)}`)
      }
      return onChain(async (server) => {
        const signerSet = await readSignerSet(rpc, treasury)
        if (signerSet === null) return refused(SALE_REFUSAL.INVALID_TREASURY, `${treasury} has no account`)
        const outsider = [...byOfficer.keys()].find((signer) => !signerSet.signers.includes(signer))
        if (outsider !== undefined) {
          return refused(SALE_REFUSAL.SIGNATURE_MISMATCH, `${outsider} is not a signer of ${treasury}`)
        }
        if (byOfficer.size < signerSet.threshold) {
          return refused(
            SALE_REFUSAL.BELOW_THRESHOLD,
            `${byOfficer.size} of the ${signerSet.threshold} signatures ${treasury} requires`,
          )
        }
        return authorized(server, closeOperation(treasury, [signed]), expirationLedger)
      })
    },

    purchaseReceipt({ buyer, transaction }) {
      const events = eventsOf(transaction.resultMeta)
      if (events === undefined) return refused(SALE_REFUSAL.UNREADABLE_TRANSACTION, 'The settled transaction could not be read')
      const purchase = events.map((event) => purchaseIn(event, address, buyer)).find((found) => found !== undefined)
      return purchase === undefined
        ? refused(SALE_REFUSAL.PURCHASE_NOT_FOUND, `No purchase by ${buyer} in this transaction`)
        : succeed(purchase, 'Purchase read from its event')
    },

    read: () =>
      onChain(async (server) => {
        const view = (method: string) =>
          simulateRead(server, passphrase, {
            source: SIMULATION_SOURCE,
            operation: Operation.invokeContractFunction({ contract: address, function: method, args: [] }),
          })
        const [state, config, accounting, contracts, finalizableAt] = await Promise.all([
          view('state'),
          view('config'),
          view('accounting'),
          view('contracts'),
          view('finalizable_at'),
        ])
        const sale = saleViewOf(state, config, accounting, contracts, finalizableAt)
        return sale === undefined
          ? refused(SALE_REFUSAL.UNEXPECTED_RESPONSE, `${address} did not answer like a sale`)
          : succeed(sale, 'Sale read from the chain')
      }),
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function saleViewOf(
  state: unknown,
  config: unknown,
  accounting: unknown,
  contracts: unknown,
  finalizableAt: unknown,
): SaleView | undefined {
  const [stateName] = Array.isArray(state) ? state : []
  const saleState = SALE_STATES[String(stateName)]
  if (saleState === undefined || !isRecord(config) || !isRecord(accounting) || !isRecord(contracts)) return undefined
  const { price } = config
  const { supply, sold_tokens: soldQuotas, raised } = accounting
  const { payment_asset: paymentAsset } = contracts
  if (
    typeof price !== 'bigint' ||
    typeof supply !== 'bigint' ||
    typeof soldQuotas !== 'bigint' ||
    typeof raised !== 'bigint' ||
    typeof paymentAsset !== 'string' ||
    typeof finalizableAt !== 'bigint'
  ) {
    return undefined
  }
  const terms = { pricePerQuota: price, supply, soldQuotas, raised, paymentAsset }
  return saleState === SALE_STATE.CLOSING
    ? { state: saleState, finalizableAt: Number(finalizableAt) * 1_000, ...terms }
    : { state: saleState, ...terms }
}
