import { defineConfig } from "hardhat/config";
import hardhatMocha from "@nomicfoundation/hardhat-mocha";
import hardhatEthers from "@nomicfoundation/hardhat-ethers";
import * as dotenv from "dotenv";

dotenv.config();

const arcRpcUrlEnv = process.env.ARC_RPC_URL?.trim();
const arcRpcUrl =
  arcRpcUrlEnv && arcRpcUrlEnv.length > 0
    ? arcRpcUrlEnv
    : "https://rpc.testnet.arc.network";
const privateKey = process.env.PRIVATE_KEY?.trim();
const accounts = privateKey && privateKey.length > 0 ? [privateKey] : [];

export default defineConfig({
  plugins: [hardhatMocha, hardhatEthers],

  solidity: "0.8.24",

  paths: {
    sources: "./contracts",
    tests: "./test",
    cache: "./cache",
    artifacts: "./artifacts",
  },

  networks: {
    arc_testnet: {
      type: "http",
      url: arcRpcUrl,
      chainId: 5042002,
      accounts,
    },
  },

  test: { mocha: { timeout: 60_000 } },
});
