# Math Rendering Test

## Inline Math

The quadratic formula is $x = \frac{-b \pm \sqrt{b^{2} - 4ac}}{2a}$.

Einstein's famous equation $E = mc^{2}$ relates energy and mass.

Greek letters: $\alpha + \beta = \gamma$ and $\pi \approx 3.14159$.

## Display Math

$$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$

$$\int_{0}^{\infty} e^{-x^{2}} dx = \frac{\sqrt{\pi}}{2}$$

$$\forall x \in \mathbb{R}: x^{2} \geq 0$$

## Operators and Arrows

Logical: $A \land B \Rightarrow C$

Set theory: $A \cup B \subseteq C$

Calculus: $\nabla f = \partial f / \partial x$

## Complex Formulas

### Fractions

$$\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$$

$$\frac{\partial f}{\partial x} = \lim_{h \to 0} \frac{f(x+h) - f(x)}{h}$$

### Matrices

$$\begin{pmatrix} a & b \\ c & d \end{pmatrix}$$

$$\begin{bmatrix} 1 & 2 & 3 \\ 4 & 5 & 6 \\ 7 & 8 & 9 \end{bmatrix}$$

## Mixed: Inline + Display

The energy $E = mc^2$ leads to:

$$E = \frac{mc^2}{\sqrt{1 - \frac{v^2}{c^2}}}$$

where $v$ is velocity and $c$ is the speed of light.

## Edge Cases

Just text: $\text{hello world}$

Unrecognized command: $\foobar$

Escaped braces: $\{x\}$

Multiple inline: $a^2 + b^2 = c^2$ and $e^{i\pi} + 1 = 0$

## Math Inside Containers

- List item with $x^2$ inline
- Another item with $\frac{1}{2}$

> Blockquote with $\alpha + \beta$ inline

| Column 1 | Column 2 |
|----------|----------|
| $x^2$    | $y^2$    |
