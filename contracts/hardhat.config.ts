import { defineConfig } from "hardhat/config";
import hardhatMocha from "@nomicfoundation/hardhat-mocha";
import hardhatEthers from "@nomicfoundation/hardhat-ethers";
import * as dotenv from "dotenv";

dotenv.config();

const arcRpcUrl = process.env.ARC_RPC_URL || "http://localhost:8545";

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
      accounts: process.env.PRIVATE_KEY ? [process.env.PRIVATE_KEY] : [],
    },
  },

  test: { mocha: { timeout: 60_000 } },
});
