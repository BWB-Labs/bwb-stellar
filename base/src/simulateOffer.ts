// Simulate deploying an offer token, configuring factories, seeding a sale,
// and letting allowlisted wallets reserve allocations using BRLA

import fs from "fs";
import path from "path";
import { ethers } from "ethers";
import type { BigNumberish } from "ethers";
import * as dotenv from "dotenv";
import process from "process";
import {
  BRLA__factory,
  OfferERC20__factory,
  OfferSaleAllowlist__factory,
  OfferTokenFactory,
  OfferTokenFactory__factory,
  OfferTokenSale,
  OfferTokenSale__factory,
  OfferTokenSaleFactory,
  OfferTokenSaleFactory__factory,
} from "./typechain";

dotenv.config();
const ERC1967ProxyArtifact = require("../out/ERC1967Proxy.sol/ERC1967Proxy.json");

function deriveSiblingPath(basePath: string, index: number) {
  const slashIndex = basePath.lastIndexOf("/");
  if (slashIndex === -1) {
    throw new Error(
      'DERIVATION_PATH must contain at least one "/" to derive additional wallets',
    );
  }
  const parent = basePath.slice(0, slashIndex);
  return `${parent}/${index}`;
}

function saleStateLabel(state: number | ethers.BigNumber) {
  const value = typeof state === "number" ? state : state.toNumber();
  switch (value) {
    case 0:
      return "Preparing";
    case 1:
      return "Active";
    case 2:
      return "Successful";
    case 3:
      return "Failed";
    default:
      return `Unknown (${value})`;
  }
}

function formatTokens(amount: BigNumberish) {
  return ethers.BigNumber.from(amount).toString();
}

const providerUrl = "http://127.0.0.1:8545";
const derivationPath = "m/44'/60'/0'/0/0";
const chainIdRaw = "31337";
const networkName = "anvil";
const mnemonic = "test test test test test test test test test test test junk";

const offerName = "Sample Offer Token";
const offerSymbol = "SAMPLE";
const offerUri = "https://example.com/offer-token";
const offerSupplyRaw = "1000000";
const brlaInitialSupplyRaw = "1000000";
const brlaPerTokenDisplay = "0.1"; // 0.1 BRLA (18 decimals) per offer token
const buyerGasTopUpEth = "0.05";

interface BuyerConfig {
  label: string;
  derivationIndex: number;
  brlaToSpend: string;
  allocationTokens: string;
}

const buyerConfigs: BuyerConfig[] = [
  {
    label: "Alice",
    derivationIndex: 1,
    brlaToSpend: "250",
    allocationTokens: "3000",
  },
  {
    label: "Bob",
    derivationIndex: 2,
    brlaToSpend: "150",
    allocationTokens: "2000",
  },
];

/**
 * Orquestra todo o cenário simulado de oferta usando BRLA e factory/proxy patterns com padrões fixos do Anvil.
 *
 * Passos principais (sem logs detalhados):
 * 1. Conecta ao Anvil via mnemonic fixo e usa o mesmo owner como transfer admin para controlar tanto o deploy quanto as transferências do token.
 * 2. Sempre implanta novas implementações base (OfferERC20, OfferTokenSale, OfferSaleAllowlist) e factories correspondentes, garantindo que cada exec
 *    gere proxies atualizáveis com lógica compartilhada sem depender de endereços externos.
 * 3. Cria o OfferERC20 pelo factory, atribui o owner como holder inicial e configura o transfer admin herdado para o fluxo da venda.
 * 4. Implanta o token BRLA local e monta a allowlist por um proxy ERC1967, dando controle ao owner e suporte a futuras upgrades.
 * 5. Semelhante à allowlist, cada venda é um proxy criado pela OfferTokenSaleFactory com parâmetros (hard cap, BRLA/token, cooldown e % min).
 * 6. Owner aprova o contrato de venda para movimentar o estoque do token e chama `seedInventory` para carregar o inventário antes das compras.
 * 7. Buyers derivados recebem ETH e BRLA, aprovam a BRLA para o contrato da venda e chamam `buyWithBrla`, seguindo as alocações configuradas.
 * 8. O script lê o estado final da venda (tokens restantes, vendidos, BRLA coletado e estado) e recomenda persistir os endereços das fábricas.
 */
async function main() {
  // 1. Configura conexão RPC e define owner/transfer admin para que o mesmo conjunto de chaves tenha acesso ao deploy e às transferências.
  let provider: ethers.providers.JsonRpcProvider;
  if (chainIdRaw) {
    const parsed = Number(chainIdRaw);
    if (Number.isNaN(parsed)) {
      throw new Error(`CHAIN_ID must be numeric when provided. Received: ${chainIdRaw}`);
    }
    provider = new ethers.providers.JsonRpcProvider(providerUrl, {
      name: networkName,
      chainId: parsed,
    });
  } else {
    provider = new ethers.providers.JsonRpcProvider(providerUrl);
  }
  try {
    const connectedNetwork = await provider.getNetwork();
    console.log(
      `Connected to network ${connectedNetwork.name} (chainId ${connectedNetwork.chainId}) at ${providerUrl}`,
    );
  } catch (error) {
    console.error(
      `Failed to connect to JSON-RPC provider at ${providerUrl}. Ensure Anvil is running (npm run anvil) or update PROVIDER_URL.`,
    );
    throw error;
  }
  const wallet = ethers.Wallet.fromMnemonic(mnemonic, derivationPath).connect(
    provider,
  );
  const ownerAddress = await wallet.getAddress();
  console.log("Deployer / owner wallet:", ownerAddress);
  const transferAdminAddress = ownerAddress;
  console.log("Transfer admin wallet:", transferAdminAddress);

  // 2. Provisiona sempre uma nova fábrica de tokens (OfferTokenFactory) apontando para a implementação ERC20 base.
  let factoryAddress: string;
  let offerFactory: OfferTokenFactory;

  const offerTokenImplementation = await new OfferERC20__factory(wallet).deploy();
  await offerTokenImplementation.deployed();
  console.log(
    "OfferERC20 implementation deployed at:",
    offerTokenImplementation.address,
  );

  const offerTokenFactoryFactory = new OfferTokenFactory__factory(wallet);
  const deployedFactory = await offerTokenFactoryFactory.deploy(
    ownerAddress,
    offerTokenImplementation.address,
  );
  await deployedFactory.deployed();
  factoryAddress = deployedFactory.address;
  console.log("OfferTokenFactory deployed at:", factoryAddress);
  offerFactory = deployedFactory;

  // 3. Cria o OfferERC20 com os parâmetros da oferta (nome, símbolo, supply não divisível, URI e transfer admin).
  const offerSupply = ethers.BigNumber.from(offerSupplyRaw);
  console.log("Preparing offer token deployment:");
  console.log("  Name:", offerName);
  console.log("  Symbol:", offerSymbol);
  console.log("  Supply:", offerSupplyRaw, "(non-divisible tokens)");
  console.log("  URI:", offerUri);

  const predictedTokenAddress = await offerFactory.callStatic.createOfferToken(
    offerName,
    offerSymbol,
    ownerAddress,
    offerSupply,
    offerUri,
    transferAdminAddress,
  );

  const createTokenTx = await offerFactory.createOfferToken(
    offerName,
    offerSymbol,
    ownerAddress,
    offerSupply,
    offerUri,
    transferAdminAddress,
  );
  console.log("createOfferToken tx submitted:", createTokenTx.hash);
  await createTokenTx.wait();
  console.log("Offer token deployed at:", predictedTokenAddress);

  const offerToken = OfferERC20__factory.connect(predictedTokenAddress, wallet);
  const ownerBalance = await offerToken.balanceOf(ownerAddress);
  console.log(
    "Owner balance after deploy:",
    formatTokens(ownerBalance),
    offerSymbol,
  );

  // 4. Implanta o token BRLA local para servir de moeda da venda e garantir liquidez para os buyers.
  const brlaFactory = new BRLA__factory(wallet);
  const brlaInitialSupply = ethers.utils.parseUnits(brlaInitialSupplyRaw, 18);
  const brlaToken = await brlaFactory.deploy(brlaInitialSupply);
  await brlaToken.deployed();
  console.log("BRLA deployed at:", brlaToken.address);

  // 5. Usa ERC1967 proxy para permitir atualizações na lógica da allowlist mantendo o mesmo endereço para os buyers.
  const proxyFactory = new ethers.ContractFactory(
    ERC1967ProxyArtifact.abi,
    ERC1967ProxyArtifact.bytecode.object ?? ERC1967ProxyArtifact.bytecode,
    wallet,
  );

  const allowlistImplementation = await new OfferSaleAllowlist__factory(
    wallet,
  ).deploy();
  await allowlistImplementation.deployed();
  console.log(
    "Allowlist implementation deployed at:",
    allowlistImplementation.address,
  );
  const allowlistInitCalldata =
    allowlistImplementation.interface.encodeFunctionData("initialize", [
      ownerAddress,
    ]);
  const allowlistProxy = await proxyFactory.deploy(
    allowlistImplementation.address,
    allowlistInitCalldata,
  );
  await allowlistProxy.deployed();
  const allowlist = OfferSaleAllowlist__factory.connect(
    allowlistProxy.address,
    wallet,
  );
  console.log("Allowlist proxy deployed at:", allowlist.address);

  // 6. Provisiona sempre uma nova OfferTokenSaleFactory e sua implementação OfferTokenSale.
  let saleFactoryAddress: string;
  let offerSaleFactory: OfferTokenSaleFactory;

  const offerSaleImplementation = await new OfferTokenSale__factory(wallet).deploy();
  await offerSaleImplementation.deployed();
  console.log(
    "OfferTokenSale implementation deployed at:",
    offerSaleImplementation.address,
  );

  const offerSaleFactoryFactory = new OfferTokenSaleFactory__factory(wallet);
  const deployedSaleFactory = await offerSaleFactoryFactory.deploy(
    ownerAddress,
    offerSaleImplementation.address,
  );
  await deployedSaleFactory.deployed();
  saleFactoryAddress = deployedSaleFactory.address;
  console.log("OfferTokenSaleFactory deployed at:", saleFactoryAddress);
  offerSaleFactory = deployedSaleFactory;

  const brlaPerToken = ethers.utils.parseUnits(brlaPerTokenDisplay, 18);
  const saleConfig = {
    hardCap: ethers.utils.parseUnits(brlaInitialSupplyRaw, 18),
    coolingOffPeriod: 5 * 24 * 60 * 60,
    minSuccessPercent: 75,
  };

  const predictedSaleAddress = await offerSaleFactory.callStatic.createOfferTokenSale(
    predictedTokenAddress,
    brlaToken.address,
    brlaPerToken,
    allowlist.address,
    ownerAddress,
    saleConfig,
  );

  const createSaleTx = await offerSaleFactory.createOfferTokenSale(
    predictedTokenAddress,
    brlaToken.address,
    brlaPerToken,
    allowlist.address,
    ownerAddress,
    saleConfig,
  );
  console.log("createOfferTokenSale tx submitted:", createSaleTx.hash);
  await createSaleTx.wait();
  console.log("OfferTokenSale proxy deployed at:", predictedSaleAddress);

  const offerSale: OfferTokenSale = OfferTokenSale__factory.connect(
    predictedSaleAddress,
    wallet,
  );

  const whitelistSaleTx = await offerToken.setTransferWhitelist(
    offerSale.address,
    true,
  );
  await whitelistSaleTx.wait();
  console.log("Offer token whitelist updated for sale contract");

  // 7. Deriva buyers adicionais a partir do mnemonic principal, garantindo BRLA e ETH para interagirem com a venda.
  const buyers = buyerConfigs.map((config) => {
    const pathForBuyer = deriveSiblingPath(
      derivationPath,
      config.derivationIndex,
    );
    const walletForBuyer = ethers.Wallet.fromMnemonic(
      mnemonic,
      pathForBuyer,
    ).connect(provider);
    return {
      ...config,
      wallet: walletForBuyer,
      brlaAmount: ethers.utils.parseUnits(config.brlaToSpend, 18),
      allocationAmount: ethers.BigNumber.from(config.allocationTokens),
    };
  });

  const buyerAddresses = buyers.map((buyer) => buyer.wallet.address);
  const allocationAmounts = buyers.map((buyer) => buyer.allocationAmount);
  const allocationFlags = buyers.map(() => true);

  // 8. Owner controla a allowlist, define alocações e aponta o sale contract como controlador autorizado.
  const ownerControllerTx = await allowlist.setController(ownerAddress, true);
  await ownerControllerTx.wait();
  console.log("Allowlist owner configured as controller");

  const allocationsTx = await allowlist.setAllocations(
    buyerAddresses,
    allocationAmounts,
    allocationFlags,
  );
  await allocationsTx.wait();
  console.log("Allowlist allocations configured for buyer wallets");

  const saleControllerTx = await allowlist.setController(
    offerSale.address,
    true,
  );
  await saleControllerTx.wait();
  console.log("Allowlist now recognizes sale contract as controller");

  // 9. Aprova o contrato de venda a movimentar o estoque (inventory) e chama seedInventory para transferir os tokens.
  const approvalForInventoryTx = await offerToken.approve(
    offerSale.address,
    offerSupply,
  );
  await approvalForInventoryTx.wait();
  console.log("Approved offer supply for sale inventory seeding");

  const seedInventoryTx = await offerSale.seedInventory();
  console.log("seedInventory tx submitted:", seedInventoryTx.hash);
  await seedInventoryTx.wait();
  const saleStateAfterSeed = saleStateLabel(await offerSale.saleState());
  console.log("Sale inventory seeded; state now:", saleStateAfterSeed);

  const saleInventoryBalance = await offerToken.balanceOf(offerSale.address);
  console.log(
    "Offer tokens held by sale after seeding:",
    formatTokens(saleInventoryBalance),
    offerSymbol,
  );

  // 10. Deposita ETH/BRLA para os buyers e executa compras via buyWithBrla, respeitando as alocações da allowlist.
  const gasTopUpAmount = ethers.utils.parseEther(buyerGasTopUpEth);
  for (const buyer of buyers) {
    const topUpTx = await wallet.sendTransaction({
      to: buyer.wallet.address,
      value: gasTopUpAmount,
    });
    await topUpTx.wait();
    const brlaFundingTx = await brlaToken.transfer(
      buyer.wallet.address,
      buyer.brlaAmount,
    );
    await brlaFundingTx.wait();
    console.log(
      `Funded ${buyer.label} (${buyer.wallet.address}) with ${buyerGasTopUpEth} ETH for gas and`,
      buyer.brlaToSpend,
      "BRLA",
    );
  }

  for (const buyer of buyers) {
    const approvalTx = await brlaToken
      .connect(buyer.wallet)
      .approve(offerSale.address, buyer.brlaAmount);
    await approvalTx.wait();
    const buyTx = await offerSale
      .connect(buyer.wallet)
      .buyWithBrla(buyer.brlaAmount);
    console.log(`buyWithBrla tx for ${buyer.label} submitted:`, buyTx.hash);
    await buyTx.wait();
    const tokensReserved = await offerSale.tokensPurchased(
      buyer.wallet.address,
    );
    const brlaSpent = await offerSale.brlaSpent(buyer.wallet.address);
    console.log(
      `${buyer.label} reserved`,
      formatTokens(tokensReserved),
      offerSymbol,
      "and spent",
      ethers.utils.formatUnits(brlaSpent, 18),
      "BRLA",
    );
  }

  // 11. Lê o estado final para conferir inventário restante, bruto de tokens vendidos e BRLA coletado antes de terminar.
  const remainingSaleInventory = await offerToken.balanceOf(offerSale.address);
  const totalSold = await offerSale.soldTokens();
  const collectedBrla = await brlaToken.balanceOf(offerSale.address);
  const totalBrlaRaised = await offerSale.totalBrlaRaised();
  const currentSaleState = saleStateLabel(await offerSale.saleState());
  console.log(
    "Sale contract remaining offer balance:",
    formatTokens(remainingSaleInventory),
    offerSymbol,
  );
  console.log(
    "Sale contract recorded sold tokens:",
    formatTokens(totalSold),
    offerSymbol,
  );
  console.log(
    "Sale contract collected BRLA:",
    ethers.utils.formatUnits(collectedBrla, 18),
    "BRLA",
  );
  console.log(
    "Sale total BRLA raised (net of refunds):",
    ethers.utils.formatUnits(totalBrlaRaised, 18),
    "BRLA",
  );
  console.log("Sale state:", currentSaleState);
  console.log("Remember to persist factory addresses if reused:");
  console.log("  OfferTokenFactory:", factoryAddress);
  console.log("  OfferTokenSaleFactory:", saleFactoryAddress);
}

main().catch((error) => {
  console.error("Error:", error);
  process.exit(1);
});
