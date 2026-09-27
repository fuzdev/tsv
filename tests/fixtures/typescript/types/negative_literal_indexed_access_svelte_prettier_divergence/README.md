# Negative literal type ahead of an index — Svelte + prettier divergence

A bare negative literal type as the object of an indexed access (`-1[K]`, `-1n['a']`,
`-1[K][K]`, `f<-1[K]>(x)`), and a pair the author wrote around such a run (`(-1[K])[]`).

## Parser divergence

tsc reads `-` as a negative literal type when a numeric or bigint literal follows, then runs its
postfix loop: `-1[K]` is an indexed access on the literal type `-1`. tsv parses that tree
(`expected_ours.json`). acorn-typescript — the parser Svelte uses — reads the literal with its
expression parser, which takes the `[K]` as a computed member: `-1[K]` is the literal type of the
expression `-(1[K])` (`expected_svelte.json`). Both parsers accept every spelling here, each as
its own program, so tsv prints each as written — a pair at the literal would hand acorn tsc's
program instead of the one it read.

The bare array form `-1[]`, which acorn rejects, cannot be an `input.*` at all: tsv repairs it to
`(-1)[]`, so it is not a fixed point. It is pinned in `tests/negative_literal_postfix_parens.rs`.

## Formatter divergence

Prettier strips the pair around the run: `(-1[K])[]` → `-1[K][]`, which acorn-typescript
rejects at the `]`, so a `<script lang="ts">` that compiled stops compiling
(`output_prettier.svelte`). The other spellings match prettier.

## Reason

Parser: tsv follows tsc, the correctness oracle for TypeScript syntax. Formatter: prettier bug,
and a Svelte-parser break.

See
[conformance_svelte.md §TypeScript Corrections](../../../../../docs/conformance_svelte.md#typescript-corrections)
and [conformance_prettier_ts.md §TypeScript](../../../../../docs/conformance_prettier_ts.md#typescript).
