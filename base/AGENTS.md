# AGENTS (contracts)

- Scope: Applies only to `contracts/` (Foundry + TypeScript scripts).

- Before running any script, have first the `nix-shell` running with the `shell.nix` file provided
- Install: `npm install` in `contracts/`.
- Build TS: `npm run build` (emits to `dist/`).
- Type-check TS: `npx tsc --noEmit`.
- Lint: none configured; if present use `npx eslint .`.
- Format: Solidity with `forge fmt`; TS via Prettier if configured.
- Run all tests: `forge test`.
- Single test (function): `forge test --match-test testDistributeTokens`.
- Single file: `forge test --match-path src/test/TokenDistributor.test.sol`.
- Contract + function (verbose): `forge test --match-contract TokenDistributorTest --match-test testDistributeTokens -vvvv`.

- Imports: external before internal; group/sort; avoid unused.
- Formatting: follow existing style (2-space TS, 4-space Solidity).
- Types: prefer explicit types; avoid `any`; Solidity: explicit visibility and `view`/`pure` as appropriate.
- Naming: Contracts/types PascalCase; functions/vars camelCase; constants UPPER_SNAKE; events PascalCase.
- Errors: Solidity `require`/custom errors with clear messages; TS throws with actionable messages.
- Tests: deterministic; use `setUp()`; name as `test*`; use `vm.expectRevert` for negative paths.
- TS runtime script: `src/distributeUSDC.ts` expects `.env` with `TOKEN_DISTRIBUTOR_ADDRESS`, `USDC_ADDRESS`, `PROVIDER_URL`, `MASTER_PRIVATE_KEY`.

- Cursor/Copilot rules: none found in this project.
