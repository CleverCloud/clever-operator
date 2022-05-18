FROM fedora:latest AS builder

WORKDIR /usr/src/clever-operator
ADD src src
ADD Cargo.toml .
ADD Cargo.lock .

RUN dnf update -y && dnf install cargo openssl-devel -y
RUN cargo build --release

FROM fedora:latest

MAINTAINER Florentin Dubois <florentin.dubois@clever-cloud.com>
LABEL name="clever-operator" \
    maintainer="Florentin Dubois <florentin.dubois@clever-cloud.com>" \
    vendor="Clever Cloud S.A.S" \
    version="v0.5.0" \
    release="1" \
    summary="A kubernetes operator that expose clever cloud's resources through custom resource definition" \
    description="A kubernetes operator that expose clever cloud's resources through custom resource definition"

RUN groupadd -g 25000 clever && useradd -u 20000 clever -g clever
USER clever:clever

COPY --from=builder /usr/src/clever-operator/target/release/clever-operator /usr/local/bin
ADD LICENSE licenses/LICENSE
CMD [ "/usr/local/bin/clever-operator" ]
