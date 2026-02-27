# Syntax Highlighting — Language Gallery

Visual test for every supported fenced code block language. Each snippet
exercises keywords, strings, comments, numbers, operators, and types so
you can verify coloring across all token categories.

---

## Rust

```rust
use std::collections::HashMap;

/// Counts word frequencies in the given text.
fn word_count(text: &str) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1;
    }
    counts
}

fn main() {
    let text = "hello world hello";
    let counts = word_count(text);
    println!("{counts:?}"); // {hello: 2, world: 1}
}
```

## Python

```python
import asyncio
from dataclasses import dataclass

@dataclass
class Point:
    x: float
    y: float

    def distance(self, other: "Point") -> float:
        """Euclidean distance between two points."""
        return ((self.x - other.x) ** 2 + (self.y - other.y) ** 2) ** 0.5

async def main():
    p1, p2 = Point(0, 0), Point(3.0, 4.0)
    print(f"Distance: {p1.distance(p2):.2f}")  # 5.00

asyncio.run(main())
```

## JavaScript

```javascript
// Debounce utility — delays fn execution until after `ms` of inactivity.
function debounce(fn, ms = 300) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn.apply(this, args), ms);
  };
}

const handleInput = debounce((event) => {
  console.log(`Search: ${event.target.value}`);
}, 250);

document.querySelector("#search").addEventListener("input", handleInput);
```

## TypeScript

```typescript
interface User {
  id: number;
  name: string;
  email: string;
  roles: readonly string[];
}

type CreateUser = Omit<User, "id">;

async function fetchUser(id: number): Promise<User | null> {
  const response = await fetch(`/api/users/${id}`);
  if (!response.ok) return null;
  return response.json() as Promise<User>;
}
```

## C

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Reverse a string in-place. */
void reverse(char *s) {
    size_t len = strlen(s);
    for (size_t i = 0; i < len / 2; i++) {
        char tmp = s[i];
        s[i] = s[len - 1 - i];
        s[len - 1 - i] = tmp;
    }
}

int main(void) {
    char buf[] = "Hello, world!";
    reverse(buf);
    printf("%s\n", buf); // !dlrow ,olleH
    return 0;
}
```

## C++

```cpp
#include <iostream>
#include <vector>
#include <algorithm>
#include <numeric>

template <typename T>
T median(std::vector<T> v) {
    // Sort a copy and return the middle element.
    std::sort(v.begin(), v.end());
    auto n = v.size();
    return (n % 2 == 0)
        ? (v[n / 2 - 1] + v[n / 2]) / static_cast<T>(2)
        : v[n / 2];
}

int main() {
    std::vector<double> data = {3.1, 1.4, 1.5, 9.2, 6.5};
    std::cout << "Median: " << median(data) << std::endl;
    return 0;
}
```

## C#

```cs
using System;
using System.Linq;

namespace Demo
{
    public record Person(string Name, int Age);

    class Program
    {
        static void Main(string[] args)
        {
            var people = new[]
            {
                new Person("Alice", 30),
                new Person("Bob", 25),
                new Person("Carol", 35),
            };

            // LINQ query — average age of people over 26.
            double avg = people
                .Where(p => p.Age > 26)
                .Average(p => p.Age);

            Console.WriteLine($"Average: {avg}");
        }
    }
}
```

## Java

```java
import java.util.List;
import java.util.stream.Collectors;

public class Main {
    record Task(String title, boolean done) {}

    public static void main(String[] args) {
        var tasks = List.of(
            new Task("Write tests", true),
            new Task("Deploy", false),
            new Task("Review PR", true)
        );

        // Filter completed tasks.
        List<String> completed = tasks.stream()
            .filter(Task::done)
            .map(Task::title)
            .collect(Collectors.toList());

        System.out.println("Done: " + completed);
    }
}
```

## Go

```go
package main

import (
	"fmt"
	"strings"
	"sync"
)

// SafeCounter is a concurrency-safe counter.
type SafeCounter struct {
	mu sync.Mutex
	v  map[string]int
}

func (c *SafeCounter) Inc(key string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.v[key]++
}

func main() {
	sc := &SafeCounter{v: make(map[string]int)}
	words := strings.Fields("one two three one two one")
	for _, w := range words {
		sc.Inc(w)
	}
	fmt.Println(sc.v) // map[one:3 three:1 two:2]
}
```

## Ruby

```ruby
# Frozen string literals for performance.
# frozen_string_literal: true

class Fibonacci
  include Enumerable

  def initialize(limit)
    @limit = limit
  end

  def each
    a, b = 0, 1
    @limit.times do
      yield a
      a, b = b, a + b
    end
  end
end

puts Fibonacci.new(10).to_a.inspect
```

## PHP

```php
<?php
declare(strict_types=1);

class UserRepository
{
    /** @var array<int, array{name: string, email: string}> */
    private array $users = [];

    public function add(string $name, string $email): int
    {
        $id = count($this->users) + 1;
        $this->users[$id] = ['name' => $name, 'email' => $email];
        return $id;
    }

    public function find(int $id): ?array
    {
        return $this->users[$id] ?? null;
    }
}

$repo = new UserRepository();
$id = $repo->add("Alice", "alice@example.com");
echo "Created user #{$id}\n";
```

## Shell / Bash

```bash
#!/usr/bin/env bash
set -euo pipefail

# Deploy script — builds, tests, and pushes a Docker image.
IMAGE="registry.example.com/myapp"
TAG="${1:-latest}"

echo "Building ${IMAGE}:${TAG}..."
docker build -t "${IMAGE}:${TAG}" .

if docker run --rm "${IMAGE}:${TAG}" npm test; then
    echo "Tests passed. Pushing..."
    docker push "${IMAGE}:${TAG}"
else
    echo "Tests failed!" >&2
    exit 1
fi
```

## PowerShell

```powershell
# Get the top 5 CPU-consuming processes and export to CSV.
$threshold = 10
$procs = Get-Process |
    Where-Object { $_.CPU -gt $threshold } |
    Sort-Object CPU -Descending |
    Select-Object -First 5 Name, CPU, Id

$procs | Export-Csv -Path ".\top-cpu.csv" -NoTypeInformation

foreach ($p in $procs) {
    Write-Host "PID $($p.Id): $($p.Name) — $([math]::Round($p.CPU, 2))s"
}
```

```ps1
function Deploy-Application {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Environment,

        [ValidateSet("Blue", "Green")]
        [string]$Slot = "Blue"
    )

    try {
        $config = Get-Content ".\config\$Environment.json" | ConvertFrom-Json
        Write-Verbose "Deploying to $Environment ($Slot)"
        # Simulate deployment
        Start-Sleep -Seconds 2
        Write-Host "Deployed $($config.AppName) v$($config.Version)" -ForegroundColor Green
    }
    catch {
        Write-Error "Deployment failed: $_"
        throw
    }
}
```

## SQL

```sql
-- Top customers by lifetime order value.
WITH customer_totals AS (
    SELECT
        c.id,
        c.name,
        c.email,
        SUM(o.amount)     AS total_spent,
        COUNT(o.id)        AS order_count,
        MAX(o.created_at)  AS last_order
    FROM customers c
    JOIN orders o ON o.customer_id = c.id
    WHERE o.status = 'completed'
    GROUP BY c.id, c.name, c.email
)
SELECT *
FROM customer_totals
WHERE total_spent > 1000.00
ORDER BY total_spent DESC
LIMIT 20;
```

## HTML

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>mdink Demo</title>
  <link rel="stylesheet" href="/styles.css">
</head>
<body>
  <header class="navbar">
    <h1>Welcome</h1>
    <nav>
      <a href="/about">About</a>
      <a href="/docs">Docs</a>
    </nav>
  </header>
  <!-- Main content -->
  <main id="app" data-version="2.0"></main>
  <script src="/app.js" defer></script>
</body>
</html>
```

## CSS

```css
:root {
  --color-bg: #2b303b;
  --color-fg: #c0c5ce;
  --radius: 8px;
}

.card {
  background: var(--color-bg);
  color: var(--color-fg);
  border-radius: var(--radius);
  padding: 1.5rem;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  transition: transform 0.2s ease;
}

.card:hover {
  transform: translateY(-2px);
}

@media (max-width: 768px) {
  .card { padding: 1rem; }
}
```

## JSON

```json
{
  "name": "mdink",
  "version": "0.1.0",
  "description": "Terminal markdown renderer",
  "keywords": ["markdown", "terminal", "ratatui"],
  "dependencies": {
    "syntect": "^5.0",
    "pulldown-cmark": "^0.10"
  },
  "config": {
    "maxFileSize": 104857600,
    "themes": ["base16-ocean.dark", "InspiredGitHub"],
    "debug": false
  }
}
```

## YAML

```yaml
# CI pipeline configuration
name: CI
on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: ["stable", "nightly"]
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust ${{ matrix.rust }}
        uses: dtolnay/rust-action@v1
        with:
          toolchain: ${{ matrix.rust }}
      - run: cargo test --all-features
      - run: cargo clippy -- -D warnings
```

## XML

```xml
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>demo</artifactId>
  <version>1.0-SNAPSHOT</version>

  <dependencies>
    <dependency>
      <groupId>junit</groupId>
      <artifactId>junit</artifactId>
      <version>4.13.2</version>
      <scope>test</scope>
    </dependency>
  </dependencies>

  <!-- Build plugins -->
  <build>
    <plugins>
      <plugin>
        <artifactId>maven-compiler-plugin</artifactId>
        <configuration>
          <source>17</source>
          <target>17</target>
        </configuration>
      </plugin>
    </plugins>
  </build>
</project>
```

## Perl

```perl
use strict;
use warnings;
use File::Find;

# Find all Rust source files and count lines.
my %stats;
find(sub {
    return unless /\.rs$/;
    open my $fh, '<', $_ or die "Cannot open $_: $!";
    my $lines = 0;
    $lines++ while <$fh>;
    close $fh;
    $stats{$File::Find::name} = $lines;
}, '.');

for my $file (sort { $stats{$b} <=> $stats{$a} } keys %stats) {
    printf "%6d  %s\n", $stats{$file}, $file;
}
```

## Lua

```lua
-- Simple stack implementation.
local Stack = {}
Stack.__index = Stack

function Stack.new()
    return setmetatable({ items = {} }, Stack)
end

function Stack:push(value)
    table.insert(self.items, value)
end

function Stack:pop()
    assert(#self.items > 0, "stack underflow")
    return table.remove(self.items)
end

function Stack:peek()
    return self.items[#self.items]
end

local s = Stack.new()
s:push(10)
s:push(20)
print(s:pop())  -- 20
print(s:peek()) -- 10
```

## Haskell

```haskell
module Main where

import Data.List (sort, group)

-- | Count occurrences of each element in a list.
frequency :: (Ord a) => [a] -> [(a, Int)]
frequency = map (\xs -> (head xs, length xs)) . group . sort

-- | Fibonacci sequence via lazy evaluation.
fibs :: [Integer]
fibs = 0 : 1 : zipWith (+) fibs (tail fibs)

main :: IO ()
main = do
    let words' = ["apple", "banana", "apple", "cherry", "banana", "apple"]
    print $ frequency words'
    print $ take 10 fibs
```

## Scala

```scala
object WordCount extends App {
  val text = "the quick brown fox jumps over the lazy dog the fox"

  // Group, count, sort descending by frequency.
  val counts = text
    .split("\\s+")
    .groupBy(identity)
    .view
    .mapValues(_.length)
    .toSeq
    .sortBy(-_._2)

  counts.foreach { case (word, n) =>
    println(f"$word%-10s $n%d")
  }
}
```

## Clojure

```clojure
(ns demo.core
  (:require [clojure.string :as str]))

;; Fibonacci with lazy sequence.
(def fibs
  (lazy-cat [0 1] (map + fibs (rest fibs))))

(defn word-frequencies [text]
  (->> (str/split text #"\s+")
       (frequencies)
       (sort-by val >)))

(defn -main [& _]
  (println "Fibs:" (take 10 fibs))
  (println "Words:" (word-frequencies "one two three one two one")))
```

## R

```r
# Statistical summary of the iris dataset.
library(ggplot2)

data(iris)

# Mean petal length per species.
means <- aggregate(Petal.Length ~ Species, data = iris, FUN = mean)
print(means)

# Quick visualization.
p <- ggplot(iris, aes(x = Sepal.Length, y = Petal.Length, color = Species)) +
  geom_point(size = 2, alpha = 0.7) +
  theme_minimal() +
  labs(title = "Iris: Sepal vs Petal Length")

ggsave("iris_plot.png", p, width = 8, height = 5)
```

## Makefile

```makefile
CC       := gcc
CFLAGS   := -Wall -Wextra -O2 -std=c17
LDFLAGS  := -lm
SRC      := $(wildcard src/*.c)
OBJ      := $(SRC:.c=.o)
TARGET   := myapp

.PHONY: all clean test

all: $(TARGET)

$(TARGET): $(OBJ)
	$(CC) $(LDFLAGS) -o $@ $^

%.o: %.c
	$(CC) $(CFLAGS) -c -o $@ $<

test: $(TARGET)
	./$(TARGET) --self-test

clean:
	rm -f $(OBJ) $(TARGET)
```

## LaTeX

```latex
\documentclass[12pt]{article}
\usepackage{amsmath, amssymb}
\usepackage[margin=1in]{geometry}

\title{Euler's Identity}
\author{mdink}
\date{\today}

\begin{document}
\maketitle

\section{The Formula}

Euler's identity connects five fundamental constants:
\[
    e^{i\pi} + 1 = 0
\]

\begin{theorem}
For all $z \in \mathbb{C}$, the exponential function satisfies:
\[
    e^z = \sum_{n=0}^{\infty} \frac{z^n}{n!}
\]
\end{theorem}

\end{document}
```

## Diff

```diff
--- a/src/highlight.rs
+++ b/src/highlight.rs
@@ -14,7 +14,7 @@
-use syntect::parsing::{Scope, SyntaxSet};
+use syntect::parsing::{Scope, SyntaxDefinition, SyntaxSet};

 impl Highlighter {
     pub fn new() -> Self {
         Self {
-            syntax_set: SyntaxSet::load_defaults_newlines(),
+            syntax_set: load_syntax_set(),
             theme_set: ThemeSet::load_defaults(),
         }
     }
```

## Erlang

```erlang
-module(counter).
-behaviour(gen_server).
-export([start_link/0, increment/0, get/0]).
-export([init/1, handle_call/3, handle_cast/2]).

start_link() ->
    gen_server:start_link({local, ?MODULE}, ?MODULE, 0, []).

init(Count) -> {ok, Count}.

increment() -> gen_server:cast(?MODULE, increment).
get()       -> gen_server:call(?MODULE, get).

handle_cast(increment, Count) -> {noreply, Count + 1}.
handle_call(get, _From, Count) -> {reply, Count, Count}.
```

## OCaml

```ocaml
(* Binary search tree with pattern matching. *)
type 'a tree =
  | Leaf
  | Node of 'a tree * 'a * 'a tree

let rec insert x = function
  | Leaf -> Node (Leaf, x, Leaf)
  | Node (l, v, r) ->
    if x < v then Node (insert x l, v, r)
    else if x > v then Node (l, v, insert x r)
    else Node (l, v, r)

let rec to_list = function
  | Leaf -> []
  | Node (l, v, r) -> to_list l @ [v] @ to_list r

let () =
  let tree = List.fold_left (fun t x -> insert x t) Leaf [5; 3; 7; 1; 4] in
  List.iter (Printf.printf "%d ") (to_list tree)
```

## D

```d
import std.stdio;
import std.algorithm;
import std.array;

// Filter and transform an array using ranges.
void main() {
    auto data = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5];

    auto result = data
        .filter!(x => x > 3)
        .map!(x => x * x)
        .array
        .sort;

    writeln("Squares of values > 3: ", result);
    writefln("Sum: %d", result.sum);
}
```

## Pascal

```pascal
program Factorial;

function Fact(n: Integer): Int64;
begin
  if n <= 1 then
    Fact := 1
  else
    Fact := n * Fact(n - 1);
end;

var
  i: Integer;
begin
  for i := 0 to 12 do
    WriteLn(i, '! = ', Fact(i));
end.
```

## Objective-C

```objc
#import <Foundation/Foundation.h>

@interface Greeter : NSObject
@property (nonatomic, copy) NSString *name;
- (NSString *)greet;
@end

@implementation Greeter
- (NSString *)greet {
    return [NSString stringWithFormat:@"Hello, %@!", self.name];
}
@end

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        Greeter *g = [[Greeter alloc] init];
        g.name = @"World";
        NSLog(@"%@", [g greet]);
    }
    return 0;
}
```

## Groovy

```groovy
// Jenkins-style pipeline DSL.
class Pipeline {
    String name
    List<String> stages = []

    void stage(String name, Closure body) {
        stages << name
        println "Running stage: ${name}"
        body()
    }

    void sh(String cmd) {
        println "  \$ ${cmd}"
    }
}

def pipeline = new Pipeline(name: "deploy")
pipeline.stage("Build") { pipeline.sh("cargo build --release") }
pipeline.stage("Test")  { pipeline.sh("cargo test") }
println "Completed ${pipeline.stages.size()} stages."
```

## Lisp

```lisp
;;; Quicksort in Common Lisp.
(defun quicksort (lst)
  "Sort a list using the quicksort algorithm."
  (if (or (null lst) (null (cdr lst)))
      lst
      (let* ((pivot (car lst))
             (rest  (cdr lst))
             (less  (remove-if-not (lambda (x) (< x pivot)) rest))
             (greater (remove-if-not (lambda (x) (>= x pivot)) rest)))
        (append (quicksort less) (list pivot) (quicksort greater)))))

(format t "~A~%" (quicksort '(3 6 8 10 1 2 1)))
;; => (1 1 2 3 6 8 10)
```

## TCL

```tcl
# Simple HTTP server mock.
proc handle_request {method path} {
    switch -exact $method {
        GET {
            if {$path eq "/"} {
                return "200 OK: Welcome"
            } elseif {$path eq "/health"} {
                return "200 OK: healthy"
            }
        }
        POST {
            return "201 Created"
        }
        default {
            return "405 Method Not Allowed"
        }
    }
    return "404 Not Found"
}

puts [handle_request GET "/"]
puts [handle_request GET "/health"]
puts [handle_request POST "/data"]
puts [handle_request DELETE "/item"]
```

## Batch File

```bat
@echo off
setlocal enabledelayedexpansion

REM Build and deploy script for Windows.
set PROJECT=myapp
set BUILD_DIR=.\build
set VERSION=1.0.0

echo Building %PROJECT% v%VERSION%...

if not exist %BUILD_DIR% mkdir %BUILD_DIR%

for %%f in (src\*.c) do (
    echo   Compiling %%f
    cl /O2 /Fe:%BUILD_DIR%\%%~nf.obj %%f
    if errorlevel 1 (
        echo ERROR: Compilation failed for %%f
        exit /b 1
    )
)

echo Build complete.
```

## reStructuredText

```rst
==================
mdink User Guide
==================

.. contents:: Table of Contents
   :depth: 2

Installation
============

Install via cargo::

    cargo install mdink

Usage
-----

Render a markdown file in the terminal:

.. code-block:: bash

   mdink README.md

.. note::

   mdink requires a terminal that supports ANSI escape codes.
```

---

## Edge Cases

### Empty code block (no language)

```
plain text in a fenced block
no highlighting expected
```

### Unknown language tag

```brainfuck
++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>
```

### Very short snippets

```rust
42
```

```python
pass
```

```sql
SELECT 1;
```

### Unicode in code

```python
# Héllo wörld — arrow → and ellipsis …
name = "日本語テスト"
print(f"こんにちは {name}")
```
