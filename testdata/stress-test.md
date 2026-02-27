# Stress Test — mdink Parser & Renderer

> **Purpose:** Exercise every parser path and layout edge case in a single
> document. Render with `cargo run -- testdata/stress-test.md` and scroll
> through the entire file looking for corruption, blank lines where none
> should be, missing bullets, misaligned table columns, or style bleed.

---

## 1. All Six Heading Levels

# H1 — LightCyan Bold
## H2 — Green Bold
### H3 — Yellow Bold
#### H4 — White Bold+Italic
##### H5 — White Bold+Italic
###### H6 — White Bold+Italic

---

## 2. Inline Formatting — Every Combination

Plain *italic* plain **bold** plain ~~strikethrough~~ plain `code` plain.

**Bold containing *italic* inside it** and *italic containing **bold** inside it*.

***Bold-italic all at once*** versus **bold** then immediately *italic* with no gap.

~~Strikethrough with **bold** and *italic* and `code` all inside~~.

`inline code` directly touching **bold** directly touching *italic*: `x`**y***z*.

**`bold code`** and *`italic code`* and ~~`struck code`~~.

A [link with **bold** text](https://example.com) and a [link with *italic*](https://example.com).

[**Bold link**](https://example.com) vs [*italic link*](https://example.com) vs [`code link`](https://example.com).

Nested emphasis: *outer italic **inner bold** back to italic* continues here.

Triple nesting: ***bold-italic with `code` inside it*** — ends cleanly.

---

## 3. Long Lines — Wrapping Stress

This paragraph is intentionally very long. It contains enough words to guarantee
that it wraps at any reasonable terminal width, from 40 columns all the way up
to 220 columns. The wrapping algorithm must preserve inline styles across the
wrap boundary. For example: this sentence has **bold text that starts before
the wrap point and must continue correctly after the line break** without the
bold attribute being lost or doubled on the continuation line.

Another long paragraph that mixes **bold**, *italic*, `inline code`, and
~~strikethrough~~ all in the same sentence so the byte-to-style map is
exercised across multiple style transitions within a single wrapped line.
The word separator must split on Unicode break properties, not just ASCII
spaces, so: extraordinarily-long-hyphenated-compound-word-that-tests-hyphen-breaks.

---

## 4. Hard Breaks and Soft Breaks

Soft break here:
line continues after soft break, treated as a space.

Hard break here:\
line starts fresh after hard break (backslash at end of previous line).

Multiple hard breaks:\
first hard break\
second hard break\
third hard break — each should render as a separate line.

Hard break inside **bold\
formatting** — the bold modifier must survive the break.

---

## 5. Thematic Breaks — Multiple Consecutive

Paragraph before first break.

---

Paragraph between breaks.

---

Paragraph between breaks again.

***

Paragraph after a `***` break (also a valid thematic break syntax).

---

## 6. Tight Lists — No Blank Lines Between Items

- Alpha
- Beta
- Gamma
- Delta

1. First
2. Second
3. Third
4. Fourth

- [ ] Unchecked task
- [x] Checked task
- [ ] Another unchecked
- [x] Another checked

---

## 7. Loose Lists — Blank Lines Between Items

Loose lists cause pulldown-cmark to wrap each item in a Paragraph event.
The InListItemParagraph state must absorb those without emitting stray blocks.

- First loose item

- Second loose item

- Third loose item with **bold** and *italic* inline styles preserved
  across what would have been a paragraph boundary

- Fourth loose item

---

## 8. Ordered Lists — Custom Start Number

42. Forty-two
43. Forty-three
44. Forty-four

99. Ninety-nine
100. One hundred (two-digit number prefix)
101. One hundred one

---

## 9. Deeply Nested Lists — 4 Levels

- Level 1 item A
  - Level 2 item A
    - Level 3 item A
      - Level 4 item A — deepest level uses ▪ bullet
      - Level 4 item B
    - Level 3 item B
  - Level 2 item B
    - Level 3 item C with **bold** text
      - Level 4 item C with `inline code`
- Level 1 item B

Ordered mixed with unordered:

1. First ordered
   - Bullet under ordered
   - Another bullet under ordered
     1. Ordered under bullet under ordered
     2. Second ordered under bullet
2. Second ordered
   - Bullet under second ordered

---

## 10. List Items with Child Blocks

A code block nested in a list item (non-list child — must be indented):

- Item with nested code:

  ```rust
  fn nested_in_list() -> &'static str {
      "this code block must be indented under the bullet"
  }
  ```

- Item with nested blockquote:

  > This block quote is a child of the list item.
  > It should appear with │ prefix, indented under the bullet.

- Item with nested blockquote containing a list:

  > - Bullet inside blockquote inside list item
  > - Second bullet inside blockquote inside list item

- Item with text, then code, then more text:

  Before code.

  ```python
  x = 42
  ```

  After code — this paragraph must not vanish.

- Item with nested ordered list then paragraph:

  1. Sub-item one
  2. Sub-item two

  Paragraph after the sub-list, still inside the parent item.

---

## 11. Block Quotes — Depth and Content Variety

> Simple single-line block quote.

> Multi-line block quote.
> Second line of the same quote.
> Third line — no blank lines, single block.

> First paragraph of a multi-paragraph quote.
>
> Second paragraph — blank `>` line creates the separator.
>
> Third paragraph with **bold**, *italic*, and `code` all in one line.

> ## Heading Inside a Block Quote
>
> Paragraph after the heading, still inside the quote. The heading style
> (color + bold) must merge with the dim+italic quote modifier.
>
> ### H3 Inside Quote
>
> Another paragraph after H3.

> Block quote containing a list:
>
> - Bullet inside quote
> - Another bullet inside quote
>   - Nested bullet inside quote — must use ◦ bullet
>
> Paragraph after the list, still inside the quote.

> Block quote containing a code block:
>
> ```bash
> echo "code inside a block quote"
> ls -la | head -20
> ```
>
> Paragraph after code inside quote.

---

## 12. Deeply Nested Block Quotes — 4 Levels

> Level 1 quote.
>
> > Level 2 quote inside level 1.
> >
> > > Level 3 quote inside level 2.
> > >
> > > > Level 4 quote inside level 3 — four │ prefixes.
> > > >
> > > > Content at maximum depth.
> > >
> > > Back to level 3.
> >
> > Back to level 2.
>
> Back to level 1.

---

## 13. Block Quote Inside a List, List Inside a Block Quote

Block quote wrapping a list:

> Here is a list inside a top-level block quote:
>
> 1. First ordered item inside quote
> 2. Second ordered item inside quote
>    - Nested bullet inside ordered inside quote
> 3. Third ordered item inside quote

List with a block quote as a child item:

- Regular item A
- Item B with nested quote:

  > Block quote inside a list item.
  > Second line of the quote.
  >
  > Second paragraph of the quote inside the list item.

- Regular item C after the quoted item

Block quote inside a list item, with a nested list inside the block quote:

- Outer item

  > List inside quote inside list:
  >
  > - Alpha in the quote
  > - Beta in the quote

- Next outer item

---

## 14. Tables — Alignment and Width Stress

### Basic 3-column table

| Name    | Score | Status   |
|---------|-------|----------|
| Alice   | 100   | Active   |
| Bob     | 85    | Active   |
| Charlie | 72    | Inactive |

### All three alignment types

| Left              | Center              | Right              |
|:------------------|:-------------------:|-------------------:|
| left-aligned text | centered text here  | right-aligned text |
| short             |       mid           |              long  |
| x                 |          y          |                  z |

### Inline formatting in table cells

| Feature        | Description                               | Status         |
|----------------|-------------------------------------------|----------------|
| **Bold header**| *italic description* with `inline code`   | ~~deprecated~~ |
| `code cell`    | A [link](https://example.com) in a cell   | **active**     |
| Plain text     | Plain description                         | Plain status   |

### Wide cells — triggers column capping

| Very Long Header That Might Overflow | Another Very Long Header That Is Also Wide | Short |
|--------------------------------------|---------------------------------------------|-------|
| This cell has a lot of content that could exceed the terminal width | So does this one — wide tables must be capped proportionally | OK    |
| Normal                               | Normal                                      | OK    |

### Single-column table

| Solo Column       |
|-------------------|
| Only one column   |
| Must still render |
| Without separator |

### Many columns (width stress)

| C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 | C9 | C10 | C11 | C12 |
|----|----|----|----|----|----|----|----|----|-----|-----|-----|
| a  | b  | c  | d  | e  | f  | g  | h  | i  | j   | k   | l   |
| 1  | 2  | 3  | 4  | 5  | 6  | 7  | 8  | 9  | 10  | 11  | 12  |

### Table with empty cells

| A     | B     | C     |
|-------|-------|-------|
| full  |       | full  |
|       | full  |       |
| full  | full  |       |
|       |       |       |

---

## 15. Unicode — Width and Rendering

### CJK characters (2 columns wide each)

Paragraph with CJK: 日本語テスト。中文测试。한국어 테스트.

Mixed CJK and ASCII in the same paragraph: the word `幅` means "width" in
Japanese. Each CJK character occupies **two** terminal columns.

### Emoji (variable width)

Emoji in text: 🦀 Rust, 🐍 Python, 🔥 hot, ✅ done, ❌ error.

Emoji in a list:
- 🦀 Rust item
- 🐍 Python item
- 🔥 Hot item
- ✅ Done item

### CJK in a table (display-width column sizing)

| 名前       | 年齢 | 都市         |
|:-----------|-----:|:-------------|
| 田中さん   |   30 | 東京         |
| スミスさん |   25 | ロンドン     |
| 이씨       |   35 | 서울         |

### Combining characters and accents

Café, naïve, résumé, Ångström, Ñoño, Ünïcödë.

Combining diacritics: e\u0301 (e + combining acute = é).

---

## 16. Images — Alt Text Only (no rendering in Phase 4)

![Alt text for an image that cannot be rendered yet](https://example.com/image.png)

![**Bold alt text** with *italic* too](https://example.com/styled.png)

An image inline within a paragraph: see ![small icon](https://example.com/icon.svg) right here.

---

## 17. Skipped / Unknown Tags

<details>
<summary>HTML details block — should be skipped gracefully</summary>
This content inside an HTML block should not appear in output.
</details>

<div class="custom">Raw HTML div — parser must skip without panic.</div>

Paragraph after the skipped HTML — must appear normally.

---

## 18. Edge Cases — Empty and Degenerate Content

### Empty list items

- Normal item
-
- Item after empty item
-

### List item with only inline code

- `only code, nothing else`
- `code` and then text
- text and then `code`

### Consecutive headings with no content between

#### Consecutive H4
##### Consecutive H5
###### Consecutive H6

### Heading immediately followed by a list

#### Items right after heading
- No blank line between heading and list
- Second item
- Third item

### Heading immediately followed by a table

#### Table right after heading
| Col A | Col B |
|-------|-------|
| val 1 | val 2 |

### Table immediately followed by a paragraph

| Key | Value |
|-----|-------|
| a   | 1     |

Paragraph immediately after table — no empty line separating them in source.

### Block quote immediately followed by list

> Quote here.

- List immediately after quote.
- Second item.

### Ordered list that continues after interruption

1. Item one
2. Item two

Paragraph break here.

3. Item three — note: pulldown-cmark restarts numbering after a paragraph;
   this is correct CommonMark behaviour, not a renderer bug.

---

## 19. Style Boundary — No Bleed Between Adjacent Blocks

**Block 1:** This paragraph has **bold text at the very end**

*Block 2:* This paragraph has *italic text at the very start*

`Block 3:` This paragraph starts with `inline code`

Normal paragraph with no formatting — must not inherit any style from above.

---

## 20. The Kitchen Sink — Everything Together

> ### Mixed heading inside quote
>
> Here is a table inside a block quote:
>
> | Item | Value |
> |------|-------|
> | A    | 1     |
> | B    | 2     |
>
> Here is a list inside the same block quote (after the table):
>
> - Bullet **one** with bold
> - Bullet *two* with italic
>   - Nested bullet with `code`
>
> And a code block inside the same block quote:
>
> ```rust
> let answer = 42;
> println!("The answer is {answer}");
> ```
>
> Final paragraph of this deeply-mixed block quote.

A list where every item exercises a different combination:

1. Plain text item — no formatting
2. **Bold** item — only bold
3. *Italic* item — only italic
4. `Code` item — only code
5. ~~Struck~~ item — only strikethrough
6. **Bold *and italic* together** in one item
7. A [linked item](https://example.com) with URL
8. Item with a very long line that will wrap to the next line when rendered
   in a narrow terminal — continuation must align with item content, not bullet
9. Item with nested content:

   ```json
   {
     "nested": "code block",
     "inside": "ordered list"
   }
   ```

10. Final item — ten items means two-digit prefix; layout must accommodate

---

*End of stress test document.*
