# mdink

> A terminal markdown renderer
>
> — because your docs deserve better than `cat`.

![color spectrum](../assets/colors.png)

## What you're looking at

This is **raw Markdown** rendered *directly* in a terminal.
No browser. No electron. Just your shell, ~~bad choices~~ and **`mdink`**.

### Syntax highlighting

```rust
fn main() {
    let mood = if coffee > 0 { "productive" } else { "zombie" };
    println!("Today I feel {mood}");
}
```

### Tables, lists, the works

| Feature           | Status |
|-------------------|--------|
| Headings h1–h6    | Bold / italic font slots |
| Code blocks       | 40+ languages |
| Inline images     | Sixel, Kitty, iTerm2 |
| Outline panel     | `o` to toggle |

- [x] Vim-style navigation
- [x] Custom JSON themes
- [ ] World domination

#### Install in one line

```bash
cargo install mdink
```

> *Built with Rust on ratatui. MIT licensed.*
> *Star it → github.com/mdink-rs/mdink*
