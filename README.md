# inet-monitoring

A small, production-friendly service that continuously monitors internet connection quality and exposes Prometheus
metrics over HTTP.

- Periodic ICMP reachability checks
- Periodic speedtest checks with [Speedtest CLI](https://www.speedtest.net/apps/cli)
- Graceful shutdown via POSIX signals
- Prometheus metrics for success, latency, and internal health
- Docker-friendly, minimal runtime configuration via env vars

## Quick start

Prerequisites:

- Rust (1.90+) and Cargo, or Docker
- Network permissions to send ICMP (may require CAP_NET_RAW or root)

Build and run locally:

```bash
cargo build --release
TARGETS="8.8.8.8,1.1.1.1" PORT=9090 DELAY=5 TIMEOUT=5 ./target/release/inet-monitoring
```

With Docker:

```bash
docker build -t inet-monitoring .
docker run --rm \
-e TARGETS="8.8.8.8,1.1.1.1" \
-e PORT=9090 \
-e DELAY=5 \
-e TIMEOUT=5 \
-p 9090:9090 \
--cap-add=NET_RAW \
inet-monitoring
```

Check metrics:

```bash
curl -s http://localhost:9090/metrics
```

## Configuration

Environment variables:

- PORT: HTTP server port (default: 9090)
- TARGETS: comma-separated list of IPs/hosts to ping (default: 8.8.8.8)
- DELAY: seconds between pings per target, float allowed (default: 5)
- TIMEOUT: per-ping timeout in seconds, float allowed (default: 5)
- SPEEDTEST_INTERVAL: seconds between speedtest checks, float allowed (default: 300)
- SPEEDTEST_TIMEOUT: timeout for speedtest checks in seconds, float allowed (default: 60)

Examples:

- TARGETS="8.8.8.8,1.1.1.1,example.com"
- DELAY="0.5"
- TIMEOUT="2"

## Metrics

All metrics are exposed in Prometheus text format on the configured HTTP port using any URL.

## License

MIT
