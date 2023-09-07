FROM rust:latest AS builder
RUN apt update && apt install --assume-yes git clang curl libssl-dev llvm libudev-dev make protobuf-compiler
RUN rustup update nightly-2023-08-31 && rustup target add wasm32-unknown-unknown --toolchain nightly-2023-08-31

WORKDIR /code
COPY . .

RUN cargo build --release

ENTRYPOINT [ "cargo" ]
