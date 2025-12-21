import { network } from "hardhat";
import { config as dotenvConfig } from "dotenv";
import { isAddress } from "ethers";

dotenvConfig();

function requireEnv(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) {
    throw new Error(`Missing required env var: ${name}`);
  }
  return value;
}

function parseMaxFeeBps(): number {
  const raw = process.env.MAX_FEE_BPS?.trim() ?? "500";
  const value = Number(raw);
  if (!Number.isInteger(value) || value < 0 || value > 2000) {
    throw new Error(`MAX_FEE_BPS must be an integer between 0 and 2000 (got ${raw})`);
  }
  return value;
}

async function main() {
  const { ethers } = await network.connect();
  const [deployer] = await ethers.getSigners();

  const usdcAddress = requireEnv("USDC_ADDRESS");
  if (!isAddress(usdcAddress)) {
    throw new Error(`USDC_ADDRESS is not a valid address: ${usdcAddress}`);
  }

  const oracleAddress = process.env.ORACLE_ADDRESS?.trim() || deployer.address;
  if (!isAddress(oracleAddress)) {
    throw new Error(`ORACLE_ADDRESS is not a valid address: ${oracleAddress}`);
  }

  const maxFeeBps = parseMaxFeeBps();

  const networkInfo = await ethers.provider.getNetwork();
  console.log("Deploying with:", {
    deployer: deployer.address,
    chainId: networkInfo.chainId.toString(),
    usdcAddress,
    oracleAddress,
    maxFeeBps,
  });

  const Drand = await ethers.getContractFactory("DrandRandomnessProvider");
  const drand = await Drand.deploy(oracleAddress);
  await drand.waitForDeployment();
  const drandAddress = await drand.getAddress();

  const Factory = await ethers.getContractFactory("RaffleFactory");
  const factory = await Factory.deploy(usdcAddress, drandAddress, maxFeeBps);
  await factory.waitForDeployment();
  const factoryAddress = await factory.getAddress();

  console.log("Deployed contracts:");
  console.log("DrandRandomnessProvider:", drandAddress);
  console.log("RaffleFactory:", factoryAddress);
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
