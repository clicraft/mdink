# mdink Math Rendering Demo

Scroll with `j`/`k` or arrows. Quit with `q`. Everything below is the **Unicode
text fallback** — no images, works in any terminal.

---

## 1. Inline vs. display

**Inline** math flows inside a sentence and stays on one line. The relation
$E = mc^2$ links energy and mass, and the quadratic roots are
$x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$ (collapsed to one line on purpose).

**Display** math gets its own block, and top-level fractions stack like a
textbook:

$$x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$$

---

## 2. Algebra

$$(x + y)^n = \sum_{k=0}^{n} \binom{n}{k} x^{n-k} y^k$$

$$a^2 - b^2 = (a - b)(a + b) \qquad e^{i\pi} + 1 = 0$$

Exponent law: $x^{m} \cdot x^{n} = x^{m+n}$ and root $\sqrt[3]{x + \sqrt{y}}$.

---

## 3. Calculus

$$f'(x) = \lim_{h \to 0} \frac{f(x+h) - f(x)}{h}$$

$$\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}$$

$$f(x) = \sum_{n=0}^{\infty} \frac{f^{(n)}(a)}{n!} (x - a)^n$$

---

## 4. Linear algebra

$$\begin{pmatrix} a & b \\ c & d \end{pmatrix}
  \begin{pmatrix} x \\ y \end{pmatrix}
  = \begin{pmatrix} ax + by \\ cx + dy \end{pmatrix}$$

$$\det(A) = \begin{vmatrix} a & b \\ c & d \end{vmatrix} = ad - bc$$

Normal equations: $\hat{\beta} = (X^T X)^{-1} X^T y$

---

## 5. Statistics & probability

$$\bar{x} = \frac{1}{n} \sum_{i=1}^{n} x_i
  \qquad
  \sigma^2 = \frac{1}{n} \sum_{i=1}^{n} (x_i - \mu)^2$$

$$P(A \mid B) = \frac{P(B \mid A)\, P(A)}{P(B)}$$

$$f(x) = \frac{1}{\sigma\sqrt{2\pi}}\, e^{-\frac{(x - \mu)^2}{2\sigma^2}}$$

---

## 6. Set theory & logic

$$\overline{A \cup B} = \bar{A} \cap \bar{B}
  \qquad A \Rightarrow B \equiv \neg A \lor B$$

$$\forall \epsilon > 0,\ \exists \delta > 0 : |x - a| < \delta$$

$$x \in \mathbb{R} \setminus \mathbb{Q}
  \qquad \emptyset \subseteq A \subseteq A \cup B$$

---

## 7. MathJax `\(...\)` and `\[...\]` delimiters

Inline with parens: \(a^2 + b^2 = c^2\) renders just like the dollar form.

Display with brackets, on its own lines:

\[
\rho_{X,Y} = \frac{\operatorname{Cov}(X, Y)}{\sigma_X \sigma_Y}
\]

---

## 8. Fail-safe behavior (this is the point)

Malformed math must **not** corrupt the rest of the page. Each line below is
broken on purpose — notice everything after it still renders fine.

Unclosed inline dollar: $x = 1 + and the sentence simply continues.

Unclosed brace inside math: $\sqrt{x + 1$ — contained to its own span.

An unmatched `\[` with no closer stays literal: \[ x = 1 (just shows a bracket).

Unclosed display block below — the heading after it still appears:

$$\frac{a}{b}

### Still here ✓

If you can read this heading and the line under it, containment works — a
missing delimiter did not swallow the document.

Literal math in code is never touched: `$x^2$`, `\(y\)`, and:

```
not math: $E = mc^2$  \[ x \]  \frac{a}{b}
```

The end.
