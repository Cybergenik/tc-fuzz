# TC-Calc Fuzzer

This is a Grammar-aware Differential Fuzzer for
[tc-calc](https://git.terminal.click/TC/tc-calc)

## Architecture

![Arch Diagram](./arch_diagram.png)

Built with [LibAFL](https://github.com/AFLplusplus/LibAFL). Uses a
Grammar-aware expression generator to produce semantically correct typed ASTs
for both corpus input generation and seed mutations. While the main purpose is
to verify correctness, we also want to find incorrect expression handling, so a
standard havoc step was added. Coverage is guided by clang's
[SanitizerCoverage](https://clang.llvm.org/docs/SanitizerCoverage.html#tracing-pcs-with-guards)
(`trace-pc-guard`) with a edge hitcounts and parallelized across cores.

1. Generator seeds the corpus with 256 random expressions (AST depth 5)
2. Scheduler picks a corpus entry, prioritizing inputs that hit rare edges
3. Mutator transforms it in two stages:
    1. Grammar-aware: compose with a generated sub-expression, wrap in parens, or negate
    2. Havoc (byte-level): 1-4 stacked mutations (bit flips, insertions, deletions, crossover)
4. Run the mutated input against both tc-calc and a Python oracle
    1. tc-calc crashes: input saved to `crashes/`
    2. Results disagree: diff saved to `diffs/`, keyed by coverage hash (smaller inputs overwrite)
    3. Both error or both agree within tolerance: discard
5. If the input produced novel edge coverage, add it to the corpus
6. Go to 2

## Run it

Run with no UBSan:
```sh
cargo run --release -- --cores all
```

Run with UBSan:

_note: running with UBSan almost doubles edge count, only works on x86_
```sh
cargo run --release --features ubsan -- --cores all
```

UBSan is optional and runs with `abort_on_error=0` -- it writes reports to `./crashes/ubsan.*` without crashing the process. ASan is omitted because tc-calc has no heap allocations.

## Diagram

Render the architecture diagram with:

```sh
d2 --theme 200 --layout elk --scale 2 \
  --elk-edgeNodeBetweenLayers=15 --elk-nodeNodeBetweenLayers=10 \
  diagram.d2 diagram.svg && rsvg-convert diagram.svg -o arch_diagram.png
```
