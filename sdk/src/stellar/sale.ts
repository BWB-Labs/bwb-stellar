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
  type PurchaseOutcome,
  type Sale,
  type SaleRefusal,
  type SaleRefusalCode,
  type SaleState,
  type SaleView,
} from '../sale.js'
import { passphraseOf, STELLAR_NETWORK, StellarChainMismatchError, type StellarChainId, type StellarRpc } from './network.js'
import { fromHex, toHex } from './signing.js'
import { scv, simulateOperation, simulateRead, SorobanSimulationError } from './soroban.js'

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

function eventsOfReceipt(resultMeta: string): Array<xdr.ContractEvent> | undefined {
  try {
    const meta = xdr.TransactionMeta.fromXDR(resultMeta, 'base64')
    return meta.type === 'v4' ? meta.v4.operations.flatMap(({ events }) => events) : undefined
  } catch {
    return undefined
  }
}

function purchaseIn(event: xdr.ContractEvent, sale: string, buyer: string): PurchaseOutcome | undefined {
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
        served === passphrase
          ? call(server)
          : Promise.resolve(refused(SALE_REFUSAL.CHAIN_MISMATCH, `The client serves "${served}", not ${chainId}`)),
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
        const simulation = await simulate(server, buyOperation(buyer, amount, []))
        if (!Api.isSimulationSuccess(simulation)) return contractRefusal(simulation.error)
        const entries = simulation.result?.auth ?? []
        const [entry] = entries
        if (
          entry === undefined ||
          entries.length !== 1 ||
          inspectAuthEntry(entry).address !== buyer ||
          !coversPurchase(entry, { sale: address, buyer, amount })
        ) {
          return refused(SALE_REFUSAL.UNEXPECTED_AUTHORIZATION, 'The purchase did not ask the buyer for one entry covering it')
        }
        const expirationLedger = simulation.latestLedger + ENTRY_LIFETIME_LEDGERS
        return succeed(
          {
            chainId,
            buyer,
            amount,
            entry: entry.toXDR('base64'),
            entryHash: toHex(hash(buildAuthorizationEntryPreimage(entry, expirationLedger, passphrase).toXDR())),
            expirationLedger,
          },
          'Purchase prepared for the buyer to sign',
        )
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
      const operation = buyOperation(buyer, amount, [signed])
      return onChain(async (server) => {
        const simulation = await simulate(server, operation)
        if (simulation.latestLedger >= expirationLedger) {
          return refused(SALE_REFUSAL.ENTRY_EXPIRED, `The entry expired at ledger ${expirationLedger}`)
        }
        if (!Api.isSimulationSuccess(simulation)) return contractRefusal(simulation.error)
        return succeed({ chainId, invocation: operation.toXDR('base64') }, 'Purchase authorized')
      })
    },

    outcome({ buyer, receipt }) {
      const events = eventsOfReceipt(receipt.resultMeta)
      if (events === undefined) return refused(SALE_REFUSAL.UNREADABLE_RECEIPT, 'The receipt could not be read')
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
        const [state, config, accounting, contracts] = await Promise.all([
          view('state'),
          view('config'),
          view('accounting'),
          view('contracts'),
        ])
        const sale = saleViewOf(state, config, accounting, contracts)
        return sale === undefined
          ? refused(SALE_REFUSAL.UNEXPECTED_RESPONSE, `${address} did not answer like a sale`)
          : succeed(sale, 'Sale read from the chain')
      }),
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function saleViewOf(state: unknown, config: unknown, accounting: unknown, contracts: unknown): SaleView | undefined {
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
    typeof paymentAsset !== 'string'
  ) {
    return undefined
  }
  return { state: saleState, pricePerQuota: price, supply, soldQuotas, raised, paymentAsset }
}
