import { Keypair, Operation } from '@stellar/stellar-sdk/base'
import { describe, expect, it } from 'vitest'

import { NETWORK } from '../src/network.js'
import {
  DEPLOYMENT_STEP,
  DEPLOYMENT_STEPS,
  deploymentDigest,
  deploymentOperation,
  offeringTerms,
  type OfferingRequest,
} from '../src/offering.js'

const PLATFORM = {
  network: NETWORK.TESTNET,
  platform: Keypair.random().publicKey(),
  platformAdmin: Keypair.random().publicKey(),
  paymentAsset: 'CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY',
  allowlist: 'CAVI4RROECISQXR4BR3O2BYH6NJFE7WI6NU6UTTHZKX3VCR6J6PWEDRQ',
  tokenWasmHash: 'a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0',
  saleWasmHash: '49edfc275630882ce01e9a190a84f0f833ad3e48183ddd17ea2487463b0af6bd',
}

const REQUEST: OfferingRequest = {
  offeringId: 'offering-1',
  treasury: Keypair.random().publicKey(),
  tokenName: 'Edifício Aurora',
  tokenSymbol: 'AURORA',
  offeringURI: 'https://bwb.example/offerings/offering-1',
  totalTokens: '1000',
  coolingOffSeconds: '432000',
}

describe('offeringTerms', () => {
  it('derives the same token and sale addresses for the same offering, deployer and network', () => {
    const terms = offeringTerms(REQUEST, PLATFORM)

    expect(offeringTerms(REQUEST, PLATFORM)).toEqual(terms)
    expect(terms.token).toMatch(/^C[A-Z2-7]{55}$/)
    expect(terms.sale).not.toBe(terms.token)
  })

  it('derives other addresses on another network', () => {
    expect(offeringTerms(REQUEST, { ...PLATFORM, network: NETWORK.MAINNET }).token).not.toBe(offeringTerms(REQUEST, PLATFORM).token)
  })
})

describe('deploymentDigest', () => {
  it('changes when any term the officers sign changes', () => {
    const terms = offeringTerms(REQUEST, PLATFORM)
    const digest = deploymentDigest(NETWORK.TESTNET, terms)

    expect(digest).toMatch(/^[0-9a-f]{64}$/)
    expect(deploymentDigest(NETWORK.TESTNET, { ...terms, totalTokens: '1001' })).not.toBe(digest)
    expect(deploymentDigest(NETWORK.TESTNET, { ...terms, treasury: Keypair.random().publicKey() })).not.toBe(digest)
    expect(deploymentDigest(NETWORK.MAINNET, terms)).not.toBe(digest)
  })
})

describe('deploymentOperation', () => {
  it('deploys the token and the sale from the platform key, grants the sale its allowlist role, then activates it', () => {
    const terms = offeringTerms(REQUEST, PLATFORM)

    const operations = DEPLOYMENT_STEPS.map((step) => Operation.fromXDRObject(deploymentOperation(step, terms)))

    expect(operations.map(({ type, source }) => [type, source])).toEqual(DEPLOYMENT_STEPS.map(() => ['invokeHostFunction', PLATFORM.platform]))
    expect(Buffer.from(deploymentOperation(DEPLOYMENT_STEP.GRANT, terms).toXDR()).includes('grant_role')).toBe(true)
    expect(Buffer.from(deploymentOperation(DEPLOYMENT_STEP.ACTIVATION, terms).toXDR()).includes('activate')).toBe(true)
  })
})
