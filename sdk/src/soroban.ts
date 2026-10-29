
import {
  Account,
  Address,
  BASE_FEE,
  hash,
  nativeToScVal,
  Operation,
  scValToNative,
  SorobanDataBuilder,
  StrKey,
  Transaction,
  TransactionBuilder,
  xdr,
} from '@stellar/stellar-sdk/base'
import { Api, assembleTransaction, type Server } from '@stellar/stellar-sdk/rpc'

export class SorobanSimulationError extends Error {}

const SIMULATION_TIME_BOUND = 0
const FEE_FIELD_LIMIT = 4_294_967_295n

export function contractSalt(label: string): Uint8Array {
  return hash(label)
}

export function contractAddressOf({
  deployer,
  salt,
  networkId,
}: {
  deployer: string
  salt: Uint8Array
  networkId: Uint8Array
}): string {
  const preimage = xdr.HashIdPreimage.envelopeTypeContractId(
    new xdr.HashIdPreimageContractId({
      networkId: Buffer.from(networkId),
      contractIdPreimage: xdr.ContractIdPreimage.contractIdPreimageFromAddress(
        new xdr.ContractIdPreimageFromAddress({
          address: Address.fromString(deployer).toScAddress(),
          salt: Buffer.from(salt),
        }),
      ),
    }),
  )
  return StrKey.encodeContract(hash(preimage.toXDR()))
}

export const scv = {
  address: (value: string): xdr.ScVal =>
    nativeToScVal(value, { type: 'address' }),
  i128: (value: bigint): xdr.ScVal => nativeToScVal(value, { type: 'i128' }),
  u32: (value: number): xdr.ScVal => nativeToScVal(value, { type: 'u32' }),
  u64: (value: bigint): xdr.ScVal => nativeToScVal(value, { type: 'u64' }),
  string: (value: string): xdr.ScVal =>
    nativeToScVal(value, { type: 'string' }),
  symbol: (value: string): xdr.ScVal =>
    nativeToScVal(value, { type: 'symbol' }),
  bool: (value: boolean): xdr.ScVal => xdr.ScVal.scvBool(value),
  vec: (values: xdr.ScVal[]): xdr.ScVal => xdr.ScVal.scvVec(values),
  struct: (fields: Record<string, xdr.ScVal>): xdr.ScVal =>
    xdr.ScVal.scvMap(
      Object.keys(fields)
        .sort()
        .map(
          (name) =>
            new xdr.ScMapEntry({
              key: xdr.ScVal.scvSymbol(name),
              val: fields[name],
            }),
        ),
    ),
}

export function createContractOperation({
  deployer,
  salt,
  wasmHash,
  constructorArgs,
}: {
  deployer: string
  salt: Uint8Array
  wasmHash: Uint8Array
  constructorArgs: xdr.ScVal[]
}): xdr.Operation {
  return Operation.createCustomContract({
    address: Address.fromString(deployer),
    salt,
    wasmHash,
    constructorArgs,
    source: deployer,
  })
}

export function invokeContractOperation({
  contract,
  method,
  args,
  source,
}: {
  contract: string
  method: string
  args: xdr.ScVal[]
  source: string
}): xdr.Operation {
  return Operation.invokeContractFunction({
    contract,
    function: method,
    args,
    source,
  })
}

export function invokesHostFunction(operation: xdr.Operation): boolean {
  return operation.body.type === 'invokeHostFunction'
}

export async function prepareSorobanTransaction(
  server: Server,
  transaction: Transaction,
): Promise<Transaction> {
  const simulation = await server.simulateTransaction(transaction)
  if (!Api.isSimulationSuccess(simulation)) {
    throw new SorobanSimulationError(simulation.error)
  }
  const assembled = assembleTransaction(transaction, simulation)
  const sorobanData = simulation.transactionData.build()
  if (BigInt(transaction.fee) + sorobanData.resourceFee <= FEE_FIELD_LIMIT) {
    return assembled.build()
  }
  const envelope = assembled
    .setSorobanData(
      new SorobanDataBuilder(sorobanData).setResourceFee(0).build(),
    )
    .build()
    .toEnvelope()
  if (envelope.type !== 'envelopeTypeTx') {
    throw new Error(`Built a ${envelope.type} envelope, not a v1 transaction`)
  }
  const { tx } = envelope.value
  return new Transaction(
    xdr.TransactionEnvelope.envelopeTypeTx(
      new xdr.TransactionV1Envelope({
        tx: new xdr.Transaction({
          ...tx,
          ext: xdr.TransactionExt.sorobanData(sorobanData),
        }),
        signatures: [],
      }),
    ),
    transaction.networkPassphrase,
  )
}

export async function simulateRead(
  server: Server,
  passphrase: string,
  { source, operation }: { source: string; operation: xdr.Operation },
): Promise<unknown> {
  const transaction = new TransactionBuilder(new Account(source, '0'), {
    fee: BASE_FEE,
    networkPassphrase: passphrase,
  })
    .addOperation(operation)
    .setTimeout(SIMULATION_TIME_BOUND)
    .build()
  const simulation = await server.simulateTransaction(transaction)
  if (!Api.isSimulationSuccess(simulation)) {
    throw new SorobanSimulationError(simulation.error)
  }
  return simulation.result ? scValToNative(simulation.result.retval) : undefined
}

export async function contractExists(
  server: Server,
  contract: string,
): Promise<boolean> {
  const { entries } = await server.getLedgerEntries(
    xdr.LedgerKey.contractData(
      new xdr.LedgerKeyContractData({
        contract: Address.fromString(contract).toScAddress(),
        key: xdr.ScVal.scvLedgerKeyContractInstance(),
        durability: xdr.ContractDataDurability.persistent,
      }),
    ),
  )
  return entries.length > 0
}
