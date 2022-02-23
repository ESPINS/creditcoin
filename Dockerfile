#Use ci-stage image
FROM ccacrtest.azurecr.io/creditcoin/ci-linux:production AS builder
ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /creditcoin-node
COPY Cargo.toml .
COPY Cargo.lock .
ADD node /creditcoin-node/node
ADD pallets /creditcoin-node/pallets
ADD primitives /creditcoin-node/primitives
ADD runtime /creditcoin-node/runtime
ADD sha3pow /creditcoin-node/sha3pow
RUN --mount=type=cache,target=/creditcoin-node/target \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/root/.cargo/registry \
    source ~/.cargo/env && cargo build --release

RUN --mount=type=cache,target=/creditcoin-node/target \
    cp /creditcoin-node/target/release/creditcoin-node /usr/local/bin/creditcoin-node

FROM ubuntu:20.04
EXPOSE 30333/tcp
EXPOSE 30333/udp
EXPOSE 9944 9933 9615
COPY --from=builder /creditcoin-node/target/release/creditcoin-node /bin/creditcoin-node
COPY chainspecs .
COPY entrypoint.sh .
COPY iconv.sh .
RUN chmod +x /entrypoint.sh
RUN chmod +x /iconv.sh
ENTRYPOINT [ "/bin/bash", "-c", "./entrypoint.sh |& ./iconv.sh" ]
