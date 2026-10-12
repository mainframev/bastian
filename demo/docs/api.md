# API reference

Every request needs a bearer token from the `[auth]` section of the config.

## Endpoints

| Method | Path | Description |
|:-------|:-----|:------------|
| `GET` | `/maps` | List maps, newest first |
| `POST` | `/maps` | Upload a map |
| `GET` | `/maps/{id}` | Fetch one map |
| `DELETE` | `/maps/{id}` | Delete a map |

## Uploading a map

```sh
curl -X POST https://atlas.local/maps \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@berlin.geojson"
```

The response describes the stored map:

```json
{
  "id": "m_42",
  "name": "berlin",
  "features": 1834,
  "created": "2026-10-12T09:30:00Z"
}
```

## From Rust

```rust
use atlas_client::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::new("https://atlas.local", std::env::var("TOKEN")?);
    for map in client.maps().await? {
        println!("{} ({} features)", map.name, map.features);
    }
    Ok(())
}
```

## Errors

Errors use standard status codes with a JSON body:

1. `401` when the token is missing or unknown
2. `404` when the map doesn't exist
3. `413` when the upload is over 50 MB

## Rate limits

Each token may make **600 requests per minute**. Over that, requests get
`429 Too Many Requests` with a `Retry-After` header.
