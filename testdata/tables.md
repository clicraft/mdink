# Tables Demo

## Simple 3×3 table

| Name  | Age | City      |
|-------|-----|-----------|
| Alice | 30  | Barcelona |
| Bob   | 25  | London    |
| Carol | 35  | Tokyo     |

## Table with alignment

| Left aligned | Center aligned | Right aligned |
|:-------------|:--------------:|--------------:|
| left         |    center      |         right |
| text here    |    middle      |          end  |

## Table with long cell content

| Feature         | Description                                          | Status    |
|-----------------|------------------------------------------------------|-----------|
| Syntax highlight| Highlights code using syntect with base16-ocean.dark | Complete  |
| Word wrap       | Wraps text to terminal width preserving inline styles| Complete  |
| Lists           | Renders bullet, ordered, task, and nested lists      | Complete  |
| Block quotes    | Renders quotes with │ prefix and dim+italic style    | Complete  |
| Tables          | Auto-calculates column widths, handles alignment     | Complete  |

## Table with many columns (width stress test)

| C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 | C9 | C10 |
|----|----|----|----|----|----|----|----|----|-----|
| a  | b  | c  | d  | e  | f  | g  | h  | i  | j   |
| 1  | 2  | 3  | 4  | 5  | 6  | 7  | 8  | 9  | 10  |

## Table with empty cells

| Col A | Col B | Col C |
|-------|-------|-------|
| full  |       | full  |
|       | full  |       |
| full  | full  |       |

## Single-column table

| Item          |
|---------------|
| Apple         |
| Banana        |
| Cherry        |

## Table followed by content

Some text before.

| Key | Value |
|-----|-------|
| a   | 1     |
| b   | 2     |

Some text after.
