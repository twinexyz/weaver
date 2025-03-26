## Foundry

**Foundry is a blazing fast, portable and modular toolkit for Ethereum application development written in Rust.**

Foundry consists of:

-   **Forge**: Ethereum testing framework (like Truffle, Hardhat and DappTools).
-   **Cast**: Swiss army knife for interacting with EVM smart contracts, sending transactions and getting chain data.
-   **Anvil**: Local Ethereum node, akin to Ganache, Hardhat Network.
-   **Chisel**: Fast, utilitarian, and verbose solidity REPL.

## Documentation

https://book.getfoundry.sh/

## Usage

### Build

```shell
$ forge build
```

### Test

```shell
$ forge test
```

### Format

```shell
$ forge fmt
```

### Gas Snapshots

```shell
$ forge snapshot
```

### Anvil

```shell
$ anvil
```

### Deploy

```shell
forge script \
    script/PrecompileCaller.s.sol:PrecompileCallerScript \
    --rpc-url <your_rpc_url> \
    --private-key <your_private_key>
```

An example private key is: `0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a` for devnet
Running this on twine-node could work as follows:
```sh
forge script \
    script/PrecompileCaller.s.sol:PrecompileCallerScript \
    --rpc-url 127.0.0.1:8570 \
    --private-key 0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a \
    --broadcast
```

### Cast

```shell
$ cast <subcommand>
```

For example:
```sh
cast send \
    0x2910E325cf29dd912E3476B61ef12F49cb931096 \
    "callPrecompile(bytes)" 0x1234 \
    --rpc-url 127.0.0.1:8570 \
    --private-key 0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a
```

### Help

```shell
$ forge --help
$ anvil --help
$ cast --help
```
