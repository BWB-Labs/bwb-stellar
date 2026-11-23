// This script deploys the BRLA token and a TokenDistributor clone owned
// by the wallet derived from the configured mnemonic. It then funds the
// distributor with BRLA and performs a sample distribution to demo wallets.

import { ethers } from "ethers";
import * as dotenv from "dotenv";
import process from "process";
import {
  BRLA__factory,
  TokenDistributor__factory,
  TokenDistributorFactory__factory,
} from "./typechain";

dotenv.config();

function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value || value.trim() === "") {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value.trim();
}

const providerUrl = requireEnv("PROVIDER_URL");
const mnemonicRaw = requireEnv("MNEMONIC");
const mnemonic = mnemonicRaw
  .replace(/^['"]|['"]$/g, "")
  .trim()
  .replace(/\s+/g, " ");
const derivationPath = process.env.DERIVATION_PATH || "m/44'/60'/0'/0/0";

async function main() {
  const provider = new ethers.providers.JsonRpcProvider(providerUrl);
  const wallet = ethers.Wallet.fromMnemonic(mnemonic, derivationPath).connect(
    provider,
  );
  const ownerAddress = await wallet.getAddress();

  console.log("Deployer / owner wallet:", ownerAddress);

  const initialSupply = ethers.utils.parseUnits("1000000", 18);
  const brla = await new BRLA__factory(wallet).deploy(initialSupply);
  await brla.deployed();
  console.log("BRLA deployed at:", brla.address);

  const distributorImplementation = await new TokenDistributor__factory(
    wallet,
  ).deploy();
  await distributorImplementation.deployed();
  console.log(
    "TokenDistributor implementation deployed at:",
    distributorImplementation.address,
  );

  const distributorFactory = await new TokenDistributorFactory__factory(wallet).deploy(
    ownerAddress,
    distributorImplementation.address,
  );
  await distributorFactory.deployed();
  console.log(
    "TokenDistributorFactory deployed at:",
    distributorFactory.address,
  );

  const distributorName = "BRLA Distributor";
  const distributorDescription = "Demo distribution contract";
  const predictedCloneAddress =
    await distributorFactory.callStatic.createTokenDistributor(
      ownerAddress,
      distributorName,
      distributorDescription,
    );
  const createCloneTx = await distributorFactory.createTokenDistributor(
    ownerAddress,
    distributorName,
    distributorDescription,
  );
  await createCloneTx.wait();
  console.log("TokenDistributor clone deployed at:", predictedCloneAddress);

  const tokenDistributor = TokenDistributor__factory.connect(
    predictedCloneAddress,
    wallet,
  );

  const recipients = [
    ethers.Wallet.createRandom().address,
    ethers.Wallet.createRandom().address,
    ethers.Wallet.createRandom().address,
  ];
  const amounts = [
    ethers.utils.parseUnits("200", 18),
    ethers.utils.parseUnits("300", 18),
    ethers.utils.parseUnits("500", 18),
  ];

  const total = amounts.reduce(
    (acc, value) => acc.add(value),
    ethers.BigNumber.from(0),
  );
  const fundTx = await brla.transfer(tokenDistributor.address, total);
  await fundTx.wait();
  console.log(
    "Funded TokenDistributor with",
    ethers.utils.formatUnits(total, 18),
    "BRLA",
  );

  const distTx = await tokenDistributor.distributeTokens(
    brla.address,
    recipients,
    amounts,
  );
  console.log("Distribute tx sent:", distTx.hash);
  await distTx.wait();
  console.log("Distribute tx mined:", distTx.hash);

  for (const recipient of recipients) {
    const balance = await brla.balanceOf(recipient);
    console.log(
      `Balance of ${recipient}: ${ethers.utils.formatUnits(balance, 18)} BRLA`,
    );
  }
}

main().catch((error) => {
  console.error("Error:", error);
  process.exit(1);
});
