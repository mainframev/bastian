# bastian sample

A paragraph with **bold**, *italic*, ~~struck~~, `inline code` and a [link](https://example.com).
It keeps going long enough that the renderer has to wrap it across more than one line at narrow widths.

## Lists

- first item
- second item with a much longer description that should wrap neatly under its own bullet marker
  - nested item
- [x] done task
- [ ] open task

1. one
2. two
10. ten

> A quote with *emphasis*.
>
> - and a list inside

## Code

```rust
fn main() {
    println!("hello");
}
```

```sh {name=greet}
echo "a very long command line that will certainly need to be wrapped when the width is small" | tr a-z A-Z
```

    indented code

---

| Tool | Language | Notes |
|------|----------|-------|
| glow | Go | the inspiration |
| bastian | Rust | reads and runs |
