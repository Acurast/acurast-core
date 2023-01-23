FROM rust:1.66
RUN apt update && apt install --assume-yes git clang curl libssl-dev llvm libudev-dev make protobuf-compiler
RUN rustup update nightly && rustup target add wasm32-unknown-unknown --toolchain nightly

WORKDIR /code
COPY . .

RUN cargo build --release

ENTRYPOINT [ "cargo" ]