FROM rust:1.90.0-alpine AS builder

WORKDIR /build
COPY . .

RUN apk add --no-cache musl-dev openssl-dev pkgconfig && \
    cargo build --release

FROM alpine:3.19

RUN apk add --no-cache openssl

COPY --from=builder /build/target/release/inet-monitoring /app

ENV PORT=9090
ENV TARGETS=10.77.77.1,8.8.8.8,google.com,youtube.com
ENV DELAY=5
ENV TIMEOUT=5

EXPOSE 9090

CMD ["/app/inet-monitoring"]
