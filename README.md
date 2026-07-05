# Streamlet

Streamlet is a standalone, stateless route-optimization service that solves
vehicle routing problems (VRPs) with time windows and multiple refill
stations, built on top of an external routing engine
([Valhalla](https://github.com/valhalla/valhalla)). It is consumer-agnostic:
any client can submit a problem and get back optimized routes. Its first
consumer is [Green Ecolution](https://green-ecolution.de).

## Endpoints

| Method | Path                    | Description                              |
| ------ | ----------------------- | ----------------------------------------- |
| POST   | `/v1/solve`              | Solve a VRP and return optimized routes   |
| GET    | `/health`                | Liveness check                            |
| GET    | `/api-docs/openapi.json` | OpenAPI schema for the API                |

> **Note:** multiple depots are accepted in a problem, but routes always
> return to the *first* depot (current semantics).

## Configuration

Streamlet is configured entirely via environment variables:

| Variable                        | Default                 | Description                                  |
| -------------------------------- | ------------------------ | --------------------------------------------- |
| `STREAMLET_ADDR`                 | `0.0.0.0:3000`           | Address/port the HTTP server binds to         |
| `STREAMLET_VALHALLA_URL`         | `http://localhost:8002`  | Base URL of the Valhalla routing engine       |
| `STREAMLET_ENGINE_TIMEOUT_MS`    | `10000`                  | Timeout for requests to the routing engine    |
| `STREAMLET_SOLVER_TIME_LIMIT_MS` | `2000`                   | Maximum solver time budget per solve request  |

## Development

The project is a Cargo workspace with two crates: `streamlet-core` (domain
model and solver) and `server` (HTTP API).

```sh
# Build and test the whole workspace
cargo build --workspace
cargo test --workspace

# Run only the API (black-box) tests
cargo test -p server --test api

# Run the Solomon VRPTW regression benchmarks (release mode, with output)
cargo test -p streamlet-core --test solomon --release -- --nocapture
```

### Nix

A Nix flake is provided for a reproducible development shell (Rust
toolchain, `pkg-config`, `openssl`, `cargo-deny`, `cargo-edit`, `bacon`,
`rust-analyzer`):

```sh
nix develop
```
