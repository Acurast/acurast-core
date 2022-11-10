# Acurast Marketplace Pallet
## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

The Acurast Marketplace Pallet extends the Acurast Pallet by resource advertisements and matching of registered jobs with suitable sources.

The Pallet exposes a number of extrinsics additional to the strongly coupled (and required) core Marketplace Pallet.

### advertise

Allows the advertisement of resources by a source. An advertisement consists of:

- A list of `pricing` options, each stating resource pricing for a selected reward type.
- The total `capacity` not to be exceeded in matching.
- A list of `allowed_consumers`.
