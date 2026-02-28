# Table Wrapping Stress Test

## Basic long cells

| Phase | What | Effort | Requires |
|-------|------|--------|----------|
| A | Entity index + backlinks (Obsidian-style graph DB for all parsed entities) | Medium | No new dependencies — pure data structure work |
| B | File co-access clustering (group files by how often they appear together in sessions) | Medium | Python + scikit-learn, batch pipeline |
| C | Tree-sitter event enrichment (parse every saved file for structural context) | High | go-tree-sitter + language grammars for 10+ languages |
| D | Session classification + predictive prefetch based on historical patterns | Medium | Builds on B's infrastructure and clustering model |
| E | Auto-generated concept hubs that link related entities across repositories | Low | Combines outputs of B, C, D into navigable maps |

## Right-aligned numbers

| Metric | Value | Description |
|--------|------:|-------------|
| Latency | 42 | Average response time in milliseconds measured across all endpoints during peak load |
| Throughput | 15000 | Requests per second sustained over a 24-hour period with no degradation in error rate |
| Uptime | 99.97 | Percentage availability calculated monthly excluding scheduled maintenance windows |

## Mixed alignment

| Left | Center | Right |
|:-----|:------:|------:|
| This text is left-aligned and should wrap normally when it exceeds the column width | This centered text should have equal padding on both sides of each wrapped line | This right-aligned text should have padding on the left side of each wrapped line |

## Single wide column

| Key | Details |
|-----|---------|
| config | The configuration file supports JSON, TOML, and YAML formats. Place it at `~/.config/app/config.json` for user-level settings or `/etc/app/config.json` for system-wide defaults. Environment variables override file settings. |
| auth | Authentication uses JWT tokens with RS256 signing. Tokens expire after 24 hours. Refresh tokens are valid for 30 days and rotate on each use to prevent replay attacks. |

## Narrow terminal stress

| A | B | C | D | E |
|---|---|---|---|---|
| This is a fairly long cell that must wrap in a very narrow column | Another long cell competing for space | Yet another | Short | The fifth column also has quite a bit of text to display |

## Styled content in cells

| Feature | Status |
|---------|--------|
| **Bold text** inside a cell that wraps to multiple lines when the description is long enough | *Italic text* that also wraps and should preserve its formatting across line breaks |
| `inline code` mixed with regular text in a cell that needs to wrap | ~~strikethrough~~ combined with **bold** and *italic* in a wrapping cell |

## Empty and minimal cells

| H1 | H2 | H3 |
|----|----|----|
| | content | |
| a | | b |
| This cell has content | | |

## Unicode and CJK

| Language | Greeting | Description |
|----------|----------|-------------|
| Japanese | こんにちは世界 | A greeting that uses full-width characters which occupy two terminal columns each |
| Korean | 안녕하세요 | Korean text also uses double-width characters and should wrap correctly |
| Emoji | 🎉🚀✨🔥 | Emoji characters are typically double-width and need correct width calculation |
