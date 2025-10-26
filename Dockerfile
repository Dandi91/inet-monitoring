FROM rust:1.90.0-alpine AS builder

WORKDIR /build
COPY . .

RUN apk add --no-cache musl-dev openssl-dev pkgconfig && \
    cargo build --release && \
    wget -O - https://install.speedtest.net/app/cli/ookla-speedtest-1.2.0-linux-x86_64.tgz | tar xz -C /tmp

FROM alpine:3.19

WORKDIR /app

RUN apk add --no-cache openssl

COPY --from=builder /build/target/release/inet-monitoring /app/inet-monitoring
COPY --from=builder /tmp/speedtest /app/speedtest

ENV PORT=9090
ENV TARGETS=8.8.8.8,google.com,youtube.com
ENV DELAY=5
ENV TIMEOUT=5

EXPOSE 9090

CMD ["/app/inet-monitoring"]
