# Endereços de contratos atuais


Os endereços dos contratos não ficam mais fixos no código do back-end. Cada
deployment do Convex escolhe qual geração de contratos usa, e qual token BRLA,
através de seis variáveis de ambiente, todas obrigatórias: um deployment do
Convex sem alguma delas falha na primeira leitura, com um erro que nomeia a
variável.

Para apontar um deployment para uma geração específica:

```
bunx convex env set OFFER_TOKEN_FACTORY_ADDRESS <endereço>
bunx convex env set OFFER_TOKEN_SALE_FACTORY_ADDRESS <endereço>
bunx convex env set TOKEN_DISTRIBUTOR_FACTORY_ADDRESS <endereço>
bunx convex env set TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS <endereço>
bunx convex env set OFFER_SALE_ALLOWLIST_ADDRESS <endereço>
bunx convex env set BRLA_ADDRESS <endereço>
```

O `deploy-wizard.sh` imprime esses seis comandos já preenchidos ao final de
um deploy, e grava os endereços em `configs/deployment-<ambiente>.json`.

Endereços de produção:

| Contrato | Endereço |
|----------|----------|
| OfferTokenFactory | `0xBC2904b8C3b6cf6C882E68F34Fe0AD633B201009` |
| OfferTokenSaleFactory | `0x8308dD18bA9927567728E3d2Ce0955D6A3D68dd5` |
| TokenDistributorFactory | `0xbaF31BAd298A93B2A62042fbFb03b50b95d2d9C0` |
| TokenDistributorByHoldingsFactory | `0xBA28a9704D81B3D88FA6CB6186eEa52394e9C781` |
| OfferSaleAllowlist | `0x8d1Ea182C4003af8eF64d1DBb286903f5Cf2E859` |
| BRLA | `0xfcb34c47f850f452c15ea1b84d51231c38a61783` |

## Configurar o servidor como controller (todas as fábricas + allowlist)

Após o deploy dos contratos, **é obrigatório configurar a carteira do servidor como controller** em todos os contratos de infraestrutura:

- OfferTokenFactory
- OfferTokenSaleFactory
- OfferSaleAllowlist
- TokenDistributorFactory
- TokenDistributorByHoldingsFactory

Para isso, use o script unificado `ConfigureFactoriesAndAllowList.s.sol`, que já está no repositório.

### Como usar

```bash
export OFFER_TOKEN_FACTORY_ADDRESS=<endereço OfferTokenFactory>
export OFFER_TOKEN_SALE_FACTORY_ADDRESS=<endereço OfferTokenSaleFactory>
export OFFER_SALE_ALLOWLIST_ADDRESS=<endereço OfferSaleAllowlist>
export TOKEN_DISTRIBUTOR_FACTORY_ADDRESS=<endereço TokenDistributorFactory>
export TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS=<endereço TokenDistributorByHoldingsFactory>

export CONTROLLER_ADDRESS=<endereço da carteira do servidor>
export CONTROLLER_ENABLED=true   # true para adicionar, false para remover

forge script script/ConfigureFactoriesAndAllowList.s.sol:ConfigureFactoriesAndAllowList \
  --rpc-url "$RPC_URL" \
  --chain "$CHAIN_ID" \
  --broadcast \
  --account deployer
```

Esse script configura o controller em **todos os contratos necessários de uma só vez**, garantindo que o backend tenha permissão para operar o fluxo de investimento corretamente.

### Ambiente em que a TokenDistributorByHoldingsFactory já existe

Num ambiente cujo deploy é anterior a esta etapa, a fábrica já está no registro
de deployment, mas a carteira do servidor ainda não é controller dela. Conceda o
controller nos cinco contratos com a keystore do dono:

```bash
./shells/manage-controller.sh <ambiente> grant <CONTROLLER_ADDRESS>
```

Depois aponte o deployment do Convex para a fábrica:

```bash
bunx convex env set TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS <endereço>
```
