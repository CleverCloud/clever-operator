FROM redhat/ubi9:9.8@sha256:46d19c10caf9888e8a01131283eaaf50c7f5d4eddab02cd92a66f8adf2e15407 AS builder

WORKDIR /usr/src/clever-kubernetes-operator
ADD crates crates
ADD Cargo.toml .
ADD Cargo.lock .
ADD rust-toolchain .

RUN dnf update -y && dnf install gcc openssl-devel -y
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --verbose -y \
    && export PATH="$HOME/.cargo/bin:$PATH" \
    && cargo build --release

FROM redhat/ubi9:9.8@sha256:46d19c10caf9888e8a01131283eaaf50c7f5d4eddab02cd92a66f8adf2e15407

LABEL name="clever-kubernetes-operator" \
    maintainer="Florentin Dubois <florentin.dubois@clever-cloud.com>" \
    vendor="Clever Cloud S.A.S" \
    version="0.8.0" \
    release="1" \
    summary="A kubernetes operator that expose clever cloud's resources through custom resource definition" \
    description="A kubernetes operator that expose clever cloud's resources through custom resource definition"

RUN groupadd -g 25000 clever && useradd -u 20000 clever -g clever
USER clever:clever

COPY --from=builder /usr/src/clever-kubernetes-operator/target/release/clever-kubernetes-operator /usr/local/bin
ADD LICENSE licenses/LICENSE
CMD [ "/usr/local/bin/clever-kubernetes-operator" ]
