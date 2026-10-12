# Getting started

Atlas runs as a single binary with a SQLite database next to it.

## Install

```sh
curl -fsSL https://example.com/atlas/install.sh | sh
atlas --version
```

## Configure

Create `atlas.toml` in the working directory:

```toml
listen = "127.0.0.1:8080"
database = "atlas.db"

[auth]
tokens = ["dev-token"]
```

## Run

```sh
atlas serve --config atlas.toml
```

> **Tip:** set `RUST_LOG=atlas=debug` to see every request.

Next, read the [API reference](../api.md).
