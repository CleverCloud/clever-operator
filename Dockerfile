FROM redhat/ubi9:latest AS builder

WORKDIR /usr/src/clever-operator
ADD src src
ADD Cargo.toml .
ADD Cargo.lock .

RUN dnf update -y && dnf install gcc openssl-devel -y
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --verbose -y \
    && export PATH="$HOME/.cargo/bin:$PATH" \
    && cargo build --release

FROM redhat/ubi9:latest

MAINTAINER Florentin Dubois <florentin.dubois@clever-cloud.com>
LABEL name="clever-operator" \
    maintainer="Florentin Dubois <florentin.dubois@clever-cloud.com>" \
    vendor="Clever Cloud S.A.S" \
    version="v0.5.1" \
    release="1" \
    summary="A kubernetes operator that expose clever cloud's resources through custom resource definition" \
    description="A kubernetes operator that expose clever cloud's resources through custom resource definition"

RUN groupadd -g 25000 clever && useradd -u 20000 clever -g clever
USER clever:clever

COPY --from=builder /usr/src/clever-operator/target/release/clever-operator /usr/local/bin
ADD LICENSE licenses/LICENSE
CMD [ "/usr/local/bin/clever-operator" ]
