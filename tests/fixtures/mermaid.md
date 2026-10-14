# Mermaid diagrams

## Flowchart

```mermaid
flowchart TD
  A[Request] --> B{Authenticated?}
  B -->|Yes| C[Handle]
  B -->|No| D[401]
  C --> E[Respond]
```

## Left to right

```mermaid
graph LR
  user[User] --> api[API] --> db[(Postgres)]
```

## Sequence

```mermaid
sequenceDiagram
  Client->>Server: GET /doc.md
  Server-->>Client: 200 OK
```

## State

```mermaid
stateDiagram-v2
  [*] --> Idle
  Idle --> Running: start
  Running --> Idle: stop
```

## Class

```mermaid
classDiagram
  class Reader {
    +scroll: usize
    +layout()
  }
  Reader --> Document
```

## Entity relationship

```mermaid
erDiagram
  AUTHOR ||--o{ POST : writes
```

## Not drawn as text

```mermaid
pie title Pets
  "Dogs" : 3
  "Cats" : 2
```

## Invalid

```mermaid
flowchart TD
  A -->
```
