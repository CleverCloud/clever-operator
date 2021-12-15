FROM rust:1.57 AS builder

WORKDIR /usr/src/clever-operator
ADD src src
ADD Cargo.toml .
ADD Cargo.lock .

RUN cargo build --release

FROM debian:bullseye-slim

RUN rm -rf /var/lib/apt/lists/*
RUN groupadd -g 25000 clever && useradd -u 20000 clever -g clever

USER clever:clever
COPY --from=builder /usr/src/clever-operator/target/release/clever-operator /usr/local/bin
CMD [ "/usr/local/bin/clever-operator" ]
