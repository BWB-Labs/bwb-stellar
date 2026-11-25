// esse script demonstra como usar o EIP2612 para dar um permit()
// para que um endereço gaste pelo outro.
//
// TODO: Use esse script quando for
// integrar no front-end a assinatura offchain

import { ethers } from "ethers";
import * as dotenv from "dotenv";
import fs from "fs";
import path from "path";

dotenv.config();

function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value || value.trim() === "") {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

// Required env vars (RPC + deployer from mnemonic)
const providerUrl = requireEnv("PROVIDER_URL");
const mnemonicRaw = requireEnv("MNEMONIC");
const mnemonic = mnemonicRaw
  .replace(/^['"]|['"]$/g, "")
  .trim()
  .replace(/\s+/g, " ");
const derivationPath = process.env.DERIVATION_PATH || "m/44'/60'/0'/0/0";

// Helpers to load Foundry artifacts from contracts/out
function loadArtifact(contractName: string) {
  const outPath = path.resolve(
    __dirname,
    "..",
    "out",
    `${contractName}.sol`,
    `${contractName}.json`,
  );
  if (!fs.existsSync(outPath)) {
    throw new Error(
      `Artifact not found for ${contractName}. Expected at ${outPath}. Did you run 'forge build' in contracts/?`,
    );
  }
  const artifact = JSON.parse(fs.readFileSync(outPath, "utf8"));
  const abi = artifact.abi;
  const bytecode: string = artifact.bytecode?.object || artifact.bytecode;
  if (!abi || !bytecode) {
    throw new Error(
      `Invalid artifact for ${contractName}: missing abi or bytecode`,
    );
  }
  return { abi, bytecode };
}

async function main() {
  const provider = new ethers.providers.JsonRpcProvider(providerUrl);
  const ownerWallet = ethers.Wallet.fromMnemonic(
    mnemonic,
    derivationPath,
  ).connect(provider);

  console.log("Owner:", await ownerWallet.getAddress());

  // Load BRLA artifact and deploy BRLA with initial supply to owner
  const brlaArtifact = loadArtifact("BRLA");
  const initialSupply = ethers.utils.parseUnits("1000", 18); // 1,000 BRLA
  const BRLAF = new ethers.ContractFactory(
    brlaArtifact.abi,
    brlaArtifact.bytecode,
    ownerWallet,
  );
  const brla = await BRLAF.deploy(initialSupply);
  await brla.deployed();
  console.log("BRLA deployed at:", brla.address);

  // Prepare a spender and a recipient (random demo addresses)
  const spenderWallet = ethers.Wallet.createRandom().connect(provider);
  const recipientWallet = ethers.Wallet.createRandom().connect(provider);

  // Fund spender with a small amount of ETH to pay gas (on testnets)
  const spenderAddress = await spenderWallet.getAddress();
  const recipientAddress = await recipientWallet.getAddress();
  const ownerAddress = await ownerWallet.getAddress();
  const fundTx = await ownerWallet.sendTransaction({
    to: spenderAddress,
    value: ethers.utils.parseEther("0.01"),
  });
  await fundTx.wait();
  console.log("Funded spender with 0.01 ETH");

  // Amount to permit/spend
  const value = ethers.utils.parseUnits("100", 18); // 100 BRLA

  // Nonce for owner
  const nonce = await brla.nonces(ownerAddress);
  // Deadline (unix timestamp)
  const deadline = Math.floor(Date.now() / 1000) + 60 * 60;

  // Build EIP-712 domain and types for ERC20Permit
  const network = await provider.getNetwork();
  const domain = {
    name: await brla.name(),
    version: "1",
    chainId: network.chainId,
    verifyingContract: brla.address,
  };

  const types = {
    Permit: [
      { name: "owner", type: "address" },
      { name: "spender", type: "address" },
      { name: "value", type: "uint256" },
      { name: "nonce", type: "uint256" },
      { name: "deadline", type: "uint256" },
    ],
  } as const;

  const message = {
    owner: ownerAddress,
    spender: spenderAddress,
    value: value.toString(),
    nonce: nonce.toString(),
    deadline: deadline,
  };

  // Sign the permit off-chain with the owner's key
  const signature = await (ownerWallet as any)._signTypedData(
    domain,
    types,
    message,
  );
  const sig = ethers.utils.splitSignature(signature);
  const r = sig.r;
  const s = sig.s;
  const v = sig.v;

  // Spender submits permit
  const brlaWithSpender = brla.connect(spenderWallet);
  const permitTx = await brlaWithSpender.permit(
    ownerAddress,
    spenderAddress,
    value,
    deadline,
    v,
    r,
    s,
  );
  await permitTx.wait();
  console.log("Permit submitted by spender:", permitTx.hash);

  // Spender executes transferFrom to move BRLA from owner to recipient
  const transferTx = await brlaWithSpender.transferFrom(
    ownerAddress,
    recipientAddress,
    value,
  );
  await transferTx.wait();
  console.log("TransferFrom mined:", transferTx.hash);

  // Check final balances
  const ownerBal = await brla.balanceOf(ownerAddress);
  const recipientBal = await brla.balanceOf(recipientAddress);
  console.log("Owner BRLA balance:", ethers.utils.formatUnits(ownerBal, 18));
  console.log(
    "Recipient BRLA balance:",
    ethers.utils.formatUnits(recipientBal, 18),
  );
}

main().catch((error) => {
  console.error("Error:", error);
  process.exit(1);
});
