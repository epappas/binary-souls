FROM rust:1.83 as builder

WORKDIR /usr/src/app

RUN USER=root cargo new --bin dasn
WORKDIR /usr/src/app/dasn

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/dasn*
RUN cargo build --release

FROM debian:buster-slim

RUN apt-get update && apt-get install -y openssl ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/dasn/target/release/dasn .

CMD ["./dasn"]
