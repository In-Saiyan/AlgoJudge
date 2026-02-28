# Question Setter Guide

This guide covers everything you need to know to author problems on AlgoJudge (Olympus).

---

## Table of Contents

1. [Overview](#overview)
2. [Problem Lifecycle](#problem-lifecycle)
3. [Creating a Problem](#creating-a-problem)
4. [Writing a Generator](#writing-a-generator)
5. [Writing a Checker (Verifier)](#writing-a-checker-verifier)
6. [Uploading Binaries](#uploading-binaries)
7. [Problem Configuration](#problem-configuration)
8. [Adding Problems to Contests](#adding-problems-to-contests)
9. [Common Pitfalls](#common-pitfalls)
10. [Full Example: Two Sum](#full-example-two-sum)

---

## Overview

Every problem in AlgoJudge requires **two compiled Linux binaries** uploaded by the setter:

| Binary       | Purpose                                                    |
|--------------|------------------------------------------------------------|
| **Generator** | Produces test case input. Called once per test case.       |
| **Checker**   | Validates a contestant's output against the input. Called once per test case per submission. |

There is **no** concept of pre-generated expected output files. The checker is the sole authority on correctness — it reads the original input and the contestant's output and decides the verdict.

> **Security note:** Both the generator and checker run inside a sandbox with network disabled, a 60-second timeout, and a 4 GB memory cap. Do not depend on network access or excessive resources.

---

## Problem Lifecycle

```
1. POST /api/v1/problems          →  Problem created (status: draft)
2. POST /api/v1/problems/{id}/generator  →  Generator binary uploaded
3. POST /api/v1/problems/{id}/checker    →  Checker binary uploaded
4. Problem becomes "ready" when both binaries are present
5. POST /api/v1/contests/{id}/problems   →  Assign to a contest
```

---

## Creating a Problem

`POST /api/v1/problems` with a JSON body:

```json
{
  "title": "Two Sum",
  "description": "Given an array and a target, find two indices that sum to the target.",
  "input_format": "Line 1: n T (array size and target)\\nLine 2: a_1 a_2 ... a_n",
  "output_format": "Two space-separated 1-indexed integers i j such that a[i]+a[j]=T",
  "constraints": "2 ≤ n ≤ 200000, -10^9 ≤ a_i ≤ 10^9, -2·10^9 ≤ T ≤ 2·10^9. Exactly one solution exists.",
  "sample_input": "4 9\n2 7 11 15",
  "sample_output": "1 2",
  "sample_explanation": "a[1] + a[2] = 2 + 7 = 9",
  "difficulty": "easy",
  "tags": ["hash-map", "two-pointer"],
  "time_limit_ms": 2000,
  "memory_limit_kb": 262144,
  "num_test_cases": 10,
  "max_score": 100,
  "partial_scoring": false,
  "is_public": false,
  "max_threads": 1,
  "network_allowed": false,
  "allowed_languages": null
}
```

Key fields:

| Field | Default | Notes |
|-------|---------|-------|
| `time_limit_ms` | 1000 | Per-test-case wall time. Max enforced by system. |
| `memory_limit_kb` | 262144 (256 MB) | Per-test-case memory cap. |
| `num_test_cases` | 10 | How many times the generator is invoked (`1` through `N`). |
| `max_threads` | 1 | Set to >1 only for multi-threaded problems. Clamped by system max (default 64). |
| `network_allowed` | false | Set to `true` only for network-based challenge problems. |
| `allowed_languages` | null (all) | Restrict to e.g. `["cpp", "python"]`. Null means all languages. |
| `partial_scoring` | false | IOI-style partial credit (requires checker exit code 7). |

---

## Writing a Generator

The generator is a **compiled Linux ELF binary** that:

1. Receives the **test case number** (1-indexed integer) as a command-line argument: `./generator 3`
2. Prints the test input to **stdout**.
3. Exits with code **0** on success.

### Contract

```
Input:  argv[1] = test case number (string, e.g. "1", "2", ..., "N")
Output: stdout  = the test input exactly as contestants will receive it
Exit:   0 = success, non-zero = generator error (judging aborts)
```

### Guidelines

- **Use the test case number as the random seed** so tests are deterministic and reproducible.
- **Scale difficulty** with the test case number: small/edge cases first (1–3), medium (4–7), stress tests last (8+).
- **Guarantee the problem's constraints** — never produce input outside the stated bounds.
- **Guarantee a valid solution exists** when the problem says so. If contestans are told "a solution always exists," the generator must ensure it.
- Keep stdout output clean — no trailing debug info, no extra blank lines (unless the problem format requires them).
- The generator has **60 seconds** to run — avoid O(n²) algorithms on huge N.

### Template (C++)

```cpp
#include <bits/stdc++.h>
using namespace std;

int main(int argc, char* argv[]) {
    if (argc < 2) {
        cerr << "Usage: generator <test_case_number>" << endl;
        return 1;
    }

    int seed = atoi(argv[1]);
    mt19937 rng(seed);

    // --- Decide test size based on seed ---
    int n;
    if (seed <= 3) {
        n = 2 + rng() % 9;          // small: [2, 10]
    } else if (seed <= 7) {
        n = 100 + rng() % 901;      // medium: [100, 1000]
    } else {
        n = 10000 + rng() % 190001; // large: [10000, 200000]
    }

    // --- Generate data ---
    // ... (problem-specific logic)

    // --- Print to stdout ---
    cout << n << "\n";
    // ...

    return 0;
}
```

### Compilation

Compile **on Linux** (or in a Linux Docker container) to produce an ELF binary:

```bash
g++ -O2 -std=c++17 -o generator generator.cpp
```

The file `generator` (no extension) is what you upload.

---

## Writing a Checker (Verifier)

The checker is a **compiled Linux ELF binary** that:

1. Receives **three file paths** as command-line arguments:
   ```
   ./checker <input_file> <contestant_output> <answer_file>
   ```
2. Reads and validates the contestant's output against the input.
3. Exits with a specific code to signal the verdict.

### Exit Codes (Testlib Convention)

| Exit Code | Meaning | When to Use |
|-----------|---------|-------------|
| **0** | Accepted (AC) | Output is correct |
| **1** | Wrong Answer (WA) | Output is incorrect |
| **2** | Presentation Error (PE) | Treated as WA — format issue |
| **3** | Checker Failure (FAIL) | Bug in the checker itself (triggers Judge Error) |
| **7** | Partial Credit | For `partial_scoring` problems — print score (0.0–1.0) to stdout |

### Contract

```
Input:
  argv[1] = path to the test input file (what the generator produced)
  argv[2] = path to the contestant's output file
  argv[3] = path to an "answer" file (in our system, this is the input file again
            since we don't pre-generate expected output — use argv[1] for input data)

Output:
  stderr = human-readable message (shown as checker comment to contestant)
  stdout = (for exit code 7 only) partial score as a float

Exit code: see table above
```

> **Note:** Because AlgoJudge uses custom checkers (not diff-based judging), `argv[3]` currently receives the input file path again. Your checker should read input from `argv[1]` and contestant output from `argv[2]`. You can safely ignore `argv[3]` or use it as a secondary reference.

### Guidelines

- **Always validate contestant output format first** (can you parse it?). If not, that's WA — don't crash.
- **Print informative messages to stderr** — they're stored as "checker comments" and help contestants debug.
- **Exit 3 (FAIL/JE)** should only occur on internal checker bugs (e.g., can't open the input file). Never use exit code 3 for wrong answers.
- The checker has **60 seconds** and **4 GB** of memory — more than enough for any verification.
- For problems with **multiple valid answers** (like Two Sum where either `1 2` or `2 1` could be valid), the checker must accept all correct outputs — never compare against a single expected answer.

### Template (C++)

```cpp
#include <bits/stdc++.h>
using namespace std;

void accept(const string& msg = "OK") {
    cerr << msg << endl;
    exit(0);
}

void wrong_answer(const string& msg) {
    cerr << msg << endl;
    exit(1);
}

void checker_error(const string& msg) {
    cerr << "CHECKER BUG: " << msg << endl;
    exit(3);
}

int main(int argc, char* argv[]) {
    if (argc < 3) {
        checker_error("Usage: checker <input> <output> [answer]");
    }

    ifstream fin(argv[1]);   // test input
    ifstream fout(argv[2]);  // contestant output

    if (!fin)  checker_error("Cannot open input file");
    if (!fout) checker_error("Cannot open contestant output file");

    // --- Read input ---
    // int n; long long T;
    // fin >> n >> T;
    // ...

    // --- Read & validate contestant output ---
    // int ci, cj;
    // if (!(fout >> ci >> cj))
    //     wrong_answer("Could not parse two integers");

    // --- Verify correctness ---
    // if (a[ci-1] + a[cj-1] != T)
    //     wrong_answer("Sum does not equal target");

    accept("Correct");
    return 0;
}
```

### Compilation

```bash
g++ -O2 -std=c++17 -o checker checker.cpp
```

Upload the `checker` ELF binary.

---

## Uploading Binaries

Binaries are uploaded via `multipart/form-data` (NOT base64) with a field named `file`.

### Generator

```
POST /api/v1/problems/{problem_id}/generator
Content-Type: multipart/form-data

file: <generator ELF binary>
```

### Checker

```
POST /api/v1/problems/{problem_id}/checker
Content-Type: multipart/form-data

file: <checker ELF binary>
```

**Size limit:** 50 MB per binary.

Binaries are stored at:
```
/mnt/data/binaries/problems/{problem_id}/generator
/mnt/data/binaries/problems/{problem_id}/checker
```

---

## Problem Configuration

### Execution Limits

| Setting | Description | System Maximum |
|---------|-------------|----------------|
| `time_limit_ms` | Wall-clock time per test case | Configurable by admin |
| `memory_limit_kb` | RSS memory cap per test case | Configurable by admin |
| `max_threads` | Max threads/processes the submission may spawn | 64 (system default) |
| `network_allowed` | Whether the submission can access the network | false recommended |

### Contest-Level Overrides

When assigning a problem to a contest, you can override per-problem settings:

```
POST /api/v1/contests/{contest_id}/problems
{
  "problem_id": "...",
  "problem_code": "A",
  "sort_order": 1,
  "time_limit_ms": 3000,        // override
  "memory_limit_kb": 524288,    // override (512 MB)
  "max_score": 200,             // override
  "max_threads": 4,             // override
  "network_allowed": false      // override
}
```

Overrides apply only within that contest; the base problem definition remains unchanged.

---

## Adding Problems to Contests

```
POST /api/v1/contests/{contest_id}/problems
{
  "problem_id": "<uuid>",
  "problem_code": "A",
  "sort_order": 1
}
```

Requirements:
- You must be the contest **owner**, a **collaborator**, or an **admin**.
- The problem must have both generator and checker uploaded (status: ready).
- `problem_code` must be unique within the contest (e.g., "A", "B", "C").

---

## Common Pitfalls

### Generator Pitfalls

| Mistake | Consequence |
|---------|-------------|
| Not using seed-based RNG | Test cases change on re-judge, breaking reproducibility |
| Generator produces input violating constraints | Contestant solutions may work "by accident" or fail unpredictably |
| Not guaranteeing a valid answer exists | Correct solutions output garbage; checker gives WA |
| Writing to stderr unnecessarily | stderr is captured but not used — keep it for errors only |
| Forgetting to flush stdout | Output may be truncated |
| Producing very large output slowly | Generator times out (60s limit) |

### Checker Pitfalls

| Mistake | Consequence |
|---------|-------------|
| Using `exit(3)` for wrong answers | Triggers "Judge Error" instead of "Wrong Answer" |
| Not handling malformed contestant output | Checker crashes → Judge Error |
| Comparing floating-point with `==` | Tiny precision differences cause WA for correct solutions |
| Only accepting one valid answer format | Correct solutions get WA (e.g., `1 2` vs `2 1`) |
| Not opening files from argv | Checker reads wrong data → wrong verdicts |
| Printing nothing to stderr | Contestants get no feedback on what went wrong |

### General Pitfalls

| Mistake | Consequence |
|---------|-------------|
| Uploading a macOS/Windows binary | Binary won't execute on the Linux judge |
| Uploading a 32-bit binary on 64-bit host | May work but memory limits behave differently |
| Not testing locally first | Broken generator/checker → every submission gets Judge Error |
| Setting `num_test_cases` too high (>100) | Slow judging; generator runs 100+ times per submission |

---

## Full Example: Two Sum

**Problem:** Given `n` integers and a target `T`, find indices `i, j` (1-indexed) with `a[i] + a[j] = T`.

### generator.cpp

```cpp
#include <bits/stdc++.h>
using namespace std;

int main(int argc, char* argv[]) {
    if (argc < 2) { cerr << "Need test number\n"; return 1; }
    int seed = atoi(argv[1]);
    mt19937 rng(seed);

    // Edge cases for seeds 1-2
    if (seed == 1) { cout << "2 5\n2 3\n"; return 0; }
    if (seed == 2) { cout << "4 0\n-3 7 3 -7\n"; return 0; }

    int n;
    long long R;
    if (seed <= 5)      { n = 2 + rng() % 9;          R = 100; }
    else if (seed <= 8) { n = 100 + rng() % 901;       R = 1000000; }
    else                { n = 50000 + rng() % 150001;   R = 1000000000LL; }

    auto randval = [&]() -> long long {
        return (long long)(rng() % (2*R+1)) - R;
    };

    vector<long long> a(n);
    for (auto& x : a) x = randval();

    // Plant the answer pair
    int i0 = rng() % n, i1;
    do { i1 = rng() % n; } while (i1 == i0);
    long long v0 = randval(), v1 = randval();
    long long T = v0 + v1;
    a[i0] = v0; a[i1] = v1;

    // Ensure uniqueness: no other pair sums to T
    set<long long> used;
    used.insert(v0); used.insert(v1);
    for (int i = 0; i < n; i++) {
        if (i == i0 || i == i1) continue;
        while (used.count(T - a[i]) || a[i] == v0 || a[i] == v1) a[i] = randval();
        used.insert(a[i]);
    }

    cout << n << " " << T << "\n";
    for (int i = 0; i < n; i++) cout << a[i] << " \n"[i+1==n];
    return 0;
}
```

### checker.cpp

```cpp
#include <bits/stdc++.h>
using namespace std;

int main(int argc, char* argv[]) {
    if (argc < 3) { cerr << "FAIL: bad args\n"; return 3; }
    ifstream fin(argv[1]), fout(argv[2]);
    if (!fin)  { cerr << "FAIL: can't open input\n";  return 3; }
    if (!fout) { cerr << "FAIL: can't open output\n"; return 3; }

    int n; long long T;
    fin >> n >> T;
    vector<long long> a(n);
    for (auto& x : a) fin >> x;

    int ci, cj;
    if (!(fout >> ci >> cj)) { cerr << "Could not read two integers\n"; return 1; }

    if (ci < 1 || ci > n || cj < 1 || cj > n) {
        cerr << "Index out of range\n"; return 1;
    }
    if (ci == cj) { cerr << "Indices must be distinct\n"; return 1; }
    if (a[ci-1] + a[cj-1] != T) {
        cerr << "a[" << ci << "]+a[" << cj << "]=" << a[ci-1]+a[cj-1] << " != " << T << "\n";
        return 1;
    }

    cerr << "OK: " << ci << " " << cj << "\n";
    return 0;
}
```

### Build and Upload

```bash
# Compile on Linux (or in a container)
g++ -O2 -std=c++17 -o generator generator.cpp
g++ -O2 -std=c++17 -o checker   checker.cpp

# Upload via API (example with curl)
curl -X POST https://judge.example.com/api/v1/problems/{id}/generator \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@generator"

curl -X POST https://judge.example.com/api/v1/problems/{id}/checker \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@checker"
```

---

## Testing Locally

Before uploading, test your generator and checker locally:

```bash
# Generate test case 1
./generator 1 > input.txt

# Run a reference solution
./reference_solution input.txt output.txt

# Verify with checker
./checker input.txt output.txt input.txt
echo "Exit code: $?"   # Should be 0

# Test wrong answer detection
echo "999 999" > bad_output.txt
./checker input.txt bad_output.txt input.txt
echo "Exit code: $?"   # Should be 1
```

Run this for all test case numbers (`1` through `num_test_cases`) to catch edge-case bugs.
