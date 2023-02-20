# Acurast
## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

Repository for the Acurast related pallets and the `acurast-p256-crypto` crate.

Please refer to the respective READMEs for more information:

## Integrations

Pallets and crates relevant for third, party integrations.

- [Acurast P256 crypto](p256-crypto/README.md): Crates providing crypto primitives to support p256 signatures in substrate
- [Acurast Fulfillment Receiver Pallet](pallets/acurast-fulfillment-receiver/README.md): Pallet meant to be integrated by other parachains to receive fulfillments from Acurast Processors
- [Acurast Proxy Pallet](pallets/proxy/README.md): Pallet meant to be integrated by other parachains to interact with the Acurast parachain

## Acurast Protocol

Acurast Protocol specific pallets.

- [Acurast Pallet](pallets/acurast/README.md): Main pallet integrated by the Acurast parachain
- [Acurast Marketplace Pallet](pallets/marketplace/README.md): Acurast marketplace functionality integrated by the Acurast parachain

## Build & Tests

Use the following command to build all the crates:

```
cargo build --release
```

Use the `-p` option to build only a specific crate:

```
cargo build -p pallet-acurast --release
```

Use the following command to run all the tests:

```
cargo test
```

Use the `-p` option to test only a specific crate:

```
cargo test -p pallet-acurast
```

### Docker

Use the following command to build using the included Dockerfile:

```
docker build -t acurast-core .
```

Once a docker image is built, it is possible to run the tests with the following command:

```
docker run acurast-core test
```