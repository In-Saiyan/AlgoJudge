# Contestant Submission Guide

Everything you need to know to submit solutions on AlgoJudge.

---

## Table of Contents

1. [How Judging Works](#how-judging-works)
2. [The Project Editor](#the-project-editor)
3. [Submission Formats](#submission-formats)
4. [I/O Convention — File Arguments](#io-convention--file-arguments)
5. [Supported Languages](#supported-languages)
6. [compile.sh and run.sh](#compilesh-and-runsh)
7. [Verdicts Explained](#verdicts-explained)
8. [Common Mistakes & How to Avoid Them](#common-mistakes--how-to-avoid-them)
9. [Limits & Restrictions](#limits--restrictions)
10. [Examples by Language](#examples-by-language)

---

## How Judging Works

```
You submit  ──►  Compilation (Sisyphus)  ──►  Execution (Minos)  ──►  Verdict
                 runs compile.sh              runs your binary          AC / WA / TLE / ...
                 30 sec timeout               against each test case
```

1. Your submission is compiled inside a Docker container using your `compile.sh`.
2. The compiled binary is executed once **per test case**, with the input file and output file paths passed as **command-line arguments**.
3. A problem-specific **checker** validates your output.
4. Judging stops on the first failing test case.

---

## The Project Editor

The web UI includes a built-in **project editor** where you write and manage your solution files. When you open a problem, the editor comes **pre-loaded with a default project** containing:

```
📁 Project
├── compile.sh    ← Pre-filled for selected language
├── run.sh        ← Pre-filled for selected language
└── main.cpp      ← Your solution (empty template)
```

### What's pre-filled

When you select a language (e.g., C++), the editor auto-generates working `compile.sh` and `run.sh` scripts for that language. **You don't need to edit these unless you have special requirements** (custom flags, multiple source files, etc.).

### What you do

1. **Select your language** from the dropdown.
2. **Write your solution** in the main source file (`main.cpp`, `main.py`, etc.).
3. **(Optional)** Add extra source files if needed — update `compile.sh` accordingly.
4. **Click Submit.** The editor packages everything into a ZIP and uploads it.

> **Tip:** You can also add additional files (headers, data files, etc.) — just make sure `compile.sh` handles them.

---

## Submission Formats

### ZIP Submission (Primary — via Project Editor)

This is the default when you use the project editor. The ZIP must contain:

| File | Required | Purpose |
|------|----------|---------|
| `compile.sh` | **Yes** | Shell script that compiles your code |
| `run.sh` | **Yes** | Shell script that runs your program |
| `main.cpp` / `main.py` / etc. | **Yes** | Your solution source file(s) |

**Rules:**
- Both `compile.sh` and `run.sh` must be at the **root** of the ZIP (not inside a subfolder).
- No symlinks allowed.
- No absolute paths (e.g., `/home/user/...`).
- No path traversal (`../`).
- Total uncompressed size must be less than **5× compressed size** (zip bomb protection).
- Maximum upload size: **10 MB default** (contest organizers may set up to 100 MB).

### Source Code Submission (Legacy)

For simple single-file solutions, you can submit raw source code via:

```
POST /api/v1/submissions
{
  "problem_id": "...",
  "contest_id": "...",
  "language": "cpp",
  "source_code": "#include <bits/stdc++.h>\n..."
}
```

The system auto-generates `compile.sh` and `run.sh` for you. This mode doesn't support multi-file projects.

---

## I/O Convention — File Arguments

> **This is the most important thing to understand.**

Your program receives input and output file paths as **command-line arguments**, not via stdin/stdout.

```
./your_binary <input_file> <output_file>
```

- **Argument 1:** Path to the input file — read your input from here.
- **Argument 2:** Path to the output file — write your answer here.

**Do NOT read from stdin. Do NOT write to stdout.** The judge passes file paths, not piped data.

### Minimal C++ Example

```cpp
#include <bits/stdc++.h>
using namespace std;

int main(int argc, char* argv[]) {
    // argv[1] = input file, argv[2] = output file
    ifstream fin(argv[1]);
    ofstream fout(argv[2]);

    int a, b;
    fin >> a >> b;
    fout << a + b << "\n";

    return 0;
}
```

### Minimal Python Example

```python
import sys

input_file = sys.argv[1]
output_file = sys.argv[2]

with open(input_file) as fin:
    a, b = map(int, fin.readline().split())

with open(output_file, 'w') as fout:
    fout.write(f"{a + b}\n")
```

### Interpreted Languages (run.sh forwarding)

For interpreted languages like Python, `run.sh` must forward the file arguments to your program:

```bash
#!/bin/bash
python3 main.py "$1" "$2"
```

The `$1` and `$2` are the input and output file paths that the judge passes. The default project editor handles this for you, but keep it in mind if you customize `run.sh`.

---

## Supported Languages

| Language | Source File | Docker Image | Compiler / Runtime |
|----------|-------------|--------------|-------------------|
| C++ | `main.cpp` | `gcc:latest` | `g++ -O2 -std=c++17` |
| C | `main.c` | `gcc:latest` | `gcc -O2 -std=c11` |
| Rust | `main.rs` | `rust:1.85` | `rustc -O` |
| Go | `main.go` | `golang:1.23` | `go build` |
| Python | `main.py` | `python:3.12` | `python3` (syntax check at compile, interpreted at run) |
| Zig | `main.zig` | `zig:0.13.0` | `zig build-exe -O ReleaseFast` |

The contest or problem may restrict allowed languages. Check the problem statement.

---

## compile.sh and run.sh

### Default compile.sh examples

**C++:**
```bash
#!/bin/bash
g++ -O2 -std=c++17 -o main main.cpp
```

**Python:**
```bash
#!/bin/bash
python3 -m py_compile main.py
```

**Rust:**
```bash
#!/bin/bash
rustc -O -o main main.rs
```

### Default run.sh examples

**Compiled languages (C++, C, Rust, Go, Zig):**
```bash
#!/bin/bash
./main "$1" "$2"
```

**Python:**
```bash
#!/bin/bash
python3 main.py "$1" "$2"
```

### Custom compile.sh

You can customize `compile.sh` for advanced use cases:

```bash
#!/bin/bash
# Multi-file C++ project with custom flags
g++ -O2 -std=c++17 -pthread -o main main.cpp utils.cpp graph.cpp -I./include
```

### Rules for compile.sh / run.sh

- Must use `#!/bin/bash` shebang (or `#!/bin/sh`).
- Must produce a binary named `main`, `a.out`, `solution`, or `run` (for compiled languages).
- **Compilation timeout: 30 seconds.** If your compile takes longer, it fails.
- **Network is disabled** during compilation — you cannot download packages at compile time.
- Windows-style line endings (`\r\n`) are automatically stripped, but avoid them anyway.

---

## Verdicts Explained

| Verdict | Code | Meaning |
|---------|------|---------|
| **Accepted** | AC | Your output is correct. |
| **Wrong Answer** | WA | Your output is incorrect. Check the checker comment for details. |
| **Time Limit Exceeded** | TLE | Your program took too long on a test case. |
| **Memory Limit Exceeded** | MLE | Your program used too much memory. |
| **Runtime Error** | RE | Your program crashed (segfault, exception, non-zero exit code). |
| **Output Limit Exceeded** | OLE | Your program wrote too much to the output file. |
| **Compilation Error** | CE | `compile.sh` failed — check the compilation log. |
| **Judge Error** | JE | Internal system error — not your fault. Contact an admin. |

### Judging Order

- Test cases are run in order (1, 2, 3, ...).
- Judging **stops on the first failure**.
- Your result shows which test case failed and why.

---

## Common Mistakes & How to Avoid Them

### 1. Reading from stdin instead of file arguments

**Wrong:**
```cpp
int n;
cin >> n;  // ❌ Judge doesn't pipe stdin
```

**Right:**
```cpp
ifstream fin(argv[1]);
int n;
fin >> n;  // ✅ Read from the input file
```

### 2. Writing to stdout instead of the output file

**Wrong:**
```cpp
cout << answer << endl;  // ❌ Judge reads from the output file, not stdout
```

**Right:**
```cpp
ofstream fout(argv[2]);
fout << answer << "\n";  // ✅ Write to the output file
```

### 3. Forgetting to flush / close the output file

In most languages, closing the file or ending the program flushes automatically. But if you're using buffered I/O in some languages, explicitly close or flush:

```python
with open(output_file, 'w') as fout:  # ✅ 'with' auto-closes
    fout.write(f"{answer}\n")
```

### 4. Hardcoded file paths

**Wrong:**
```cpp
ifstream fin("input.txt");  // ❌ Hardcoded path
```

**Right:**
```cpp
ifstream fin(argv[1]);  // ✅ Use the path the judge provides
```

### 5. Missing compile.sh or run.sh

The ZIP upload is rejected immediately if either file is missing. The project editor always includes them — don't delete them.

### 6. compile.sh that doesn't produce a recognized binary

The system looks for binaries named: `main`, `a.out`, `solution`, or `run`. If your `compile.sh` produces `my_program`, it won't be found. Rename it:

```bash
#!/bin/bash
g++ -O2 -o main my_solver.cpp   # ✅ Output named 'main'
```

### 7. Non-zero exit code on success

Your program **must exit with code 0** when it runs successfully. A non-zero exit code is treated as a Runtime Error, even if the output is correct.

```cpp
int main(int argc, char* argv[]) {
    // ... solve ...
    return 0;  // ✅ Always return 0
}
```

### 8. Printing debug output to the output file

```cpp
fout << "DEBUG: n=" << n << "\n";  // ❌ Checker sees this as wrong answer
fout << answer << "\n";
```

Remove all debug output before submitting. Debug to **stderr** if needed — stderr is not checked.

### 9. Windows line endings in shell scripts

If you edit `compile.sh` on Windows, it may get `\r\n` line endings. The system auto-strips these, but to be safe, keep your editor set to Unix line endings (LF).

### 10. run.sh not forwarding arguments (interpreted languages)

If you customize `run.sh` for Python/interpreted languages, you **must** pass `$1` and `$2`:

```bash
#!/bin/bash
python3 main.py "$1" "$2"    # ✅ Forwards file paths
```

**Not:**
```bash
#!/bin/bash
python3 main.py               # ❌ Program gets no file paths
```

---

## Limits & Restrictions

### Compilation (Sisyphus)

| Limit | Value |
|-------|-------|
| Timeout | 30 seconds |
| Memory | 2 GB |
| CPU | 2 cores |
| Disk | 500 MB |
| Network | **Disabled** |

### Execution (Minos)

| Limit | Value |
|-------|-------|
| Time | Per-problem (see problem statement, e.g. 2000 ms) |
| Memory | Per-problem (see problem statement, e.g. 256 MB) |
| CPU | Single core (unless problem allows multi-threading) |
| Output size | System-configured (typically 64 MB) |
| Network | **Disabled** |
| Threads | 1 (unless problem specifies `max_threads > 1`) |

### Upload

| Limit | Value |
|-------|-------|
| Default max size | 10 MB |
| Contest max (if configured) | Up to 100 MB |
| ZIP bomb protection | Uncompressed < 5× compressed |

### Sandbox Restrictions

Your code runs in a fully isolated sandbox:
- **No network access** (not even localhost).
- **No fork/exec** beyond what your runtime needs.
- **Process isolation** — you cannot see other processes.
- **Read-only filesystem** except for the output file path.

---

## Examples by Language

### C++

```cpp
// main.cpp
#include <bits/stdc++.h>
using namespace std;

int main(int argc, char* argv[]) {
    ifstream fin(argv[1]);
    ofstream fout(argv[2]);

    int n;
    long long T;
    fin >> n >> T;

    vector<long long> a(n);
    for (auto& x : a) fin >> x;

    unordered_map<long long, int> seen;
    for (int i = 0; i < n; i++) {
        long long need = T - a[i];
        if (seen.count(need)) {
            fout << seen[need]+1 << " " << i+1 << "\n";
            return 0;
        }
        seen[a[i]] = i;
    }

    return 0;
}
```

```bash
# compile.sh
#!/bin/bash
g++ -O2 -std=c++17 -o main main.cpp
```

```bash
# run.sh
#!/bin/bash
./main "$1" "$2"
```

### Python

```python
# main.py
import sys

def solve(input_file, output_file):
    with open(input_file) as fin:
        n, T = map(int, fin.readline().split())
        a = list(map(int, fin.readline().split()))

    seen = {}
    for i, x in enumerate(a):
        need = T - x
        if need in seen:
            with open(output_file, 'w') as fout:
                fout.write(f"{seen[need]+1} {i+1}\n")
            return
        seen[x] = i

if __name__ == "__main__":
    solve(sys.argv[1], sys.argv[2])
```

```bash
# compile.sh
#!/bin/bash
python3 -m py_compile main.py
```

```bash
# run.sh
#!/bin/bash
python3 main.py "$1" "$2"
```

### Rust

```rust
// main.rs
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;

fn main() {
    let args: Vec<String> = env::args().collect();
    let input = fs::read_to_string(&args[1]).unwrap();
    let mut lines = input.lines();

    let first: Vec<i64> = lines.next().unwrap()
        .split_whitespace().map(|x| x.parse().unwrap()).collect();
    let (n, t) = (first[0] as usize, first[1]);

    let a: Vec<i64> = lines.next().unwrap()
        .split_whitespace().map(|x| x.parse().unwrap()).collect();

    let mut seen = HashMap::new();
    let mut out = fs::File::create(&args[2]).unwrap();

    for i in 0..n {
        let need = t - a[i];
        if let Some(&j) = seen.get(&need) {
            writeln!(out, "{} {}", j + 1, i + 1).unwrap();
            return;
        }
        seen.insert(a[i], i);
    }
}
```

```bash
# compile.sh
#!/bin/bash
rustc -O -o main main.rs
```

```bash
# run.sh
#!/bin/bash
./main "$1" "$2"
```

### Go

```go
// main.go
package main

import (
    "bufio"
    "fmt"
    "os"
)

func main() {
    fin, _ := os.Open(os.Args[1])
    fout, _ := os.Create(os.Args[2])
    defer fin.Close()
    defer fout.Close()

    reader := bufio.NewReader(fin)
    var n int
    var t int64
    fmt.Fscan(reader, &n, &t)

    a := make([]int64, n)
    for i := range a {
        fmt.Fscan(reader, &a[i])
    }

    seen := make(map[int64]int)
    for i := 0; i < n; i++ {
        need := t - a[i]
        if j, ok := seen[need]; ok {
            fmt.Fprintf(fout, "%d %d\n", j+1, i+1)
            return
        }
        seen[a[i]] = i
    }
}
```

```bash
# compile.sh
#!/bin/bash
go build -o main main.go
```

```bash
# run.sh
#!/bin/bash
./main "$1" "$2"
```

---

## Quick Checklist Before Submitting

- [ ] Read input from `argv[1]` (file), not stdin
- [ ] Write output to `argv[2]` (file), not stdout  
- [ ] Program exits with code 0 on success
- [ ] No debug output in the output file
- [ ] `compile.sh` produces a binary named `main` (or `solution` / `a.out` / `run`)
- [ ] `run.sh` forwards `"$1"` and `"$2"` for interpreted languages
- [ ] Output format matches exactly what the problem asks for
- [ ] Solution handles edge cases (minimum input, maximum input)
- [ ] No hardcoded file paths
