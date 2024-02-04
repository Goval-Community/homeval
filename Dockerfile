FROM debian:buster-slim as base

RUN apt update
RUN apt install -y \
    openssl

# Setup the rust environment

FROM base as setup

RUN apt-get install -y \
    build-essential \
    curl \
    libssl-dev \
    pkg-config \
    protobuf-compiler
RUN apt-get update

RUN curl https://sh.rustup.rs -sSf | sh -s -- --profile minimal --default-toolchain nightly -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup update

RUN USER=root cargo new --bin app
WORKDIR /app

# Build the project

FROM setup as build

COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./migration ./migration
COPY ./entity ./entity
COPY ./services ./services
COPY ./protobuf ./protobuf

RUN cargo build --locked --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/homeval*
RUN cargo install --path .

# Runtime

FROM base as runtime

COPY --from=build /app/target/release/homeval .

ENTRYPOINT [ "./homeval" ]
