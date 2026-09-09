# param_decorator_outer_mode_prettier_divergence

A **parameter's** decorator list parses under the *enclosing* mode, not under a class's.
The `goal` marker selects `Goal::Script` and the file declares no `"use strict"`, so the
script stays sloppy and the legacy octal inside each decorator expression is legal.

The positions covered are the non-class ones acorn's `parseAssignableListItem` reaches — a
function declaration, an object-literal method, and an ambient `declare function` — plus a
class method, whose parameter decorator is strict because the **class** is, not because it
is a decorator.

## What this pins

A decorator list is strict code when it belongs to a class: the decorators proposal puts
`DecoratorList` inside `ClassDeclaration` / `ClassExpression`, and every part of a class is
strict (ecma262 sec-strict-mode-code), so `@dec(010) class C {}` is an error even in a
sloppy script — pinned next door by
[class_decorator_octal](../class_decorator_octal_svelte_divergence/).

A **parameter** decorator is not part of a class. It belongs to the parameter list of
whatever function encloses it, and a parameter list parses under the outer mode — the same
rule [nonsimple_params_directive](../nonsimple_params_directive_svelte_prettier_divergence/)
pins for a parameter *default*. So the strictness of `@dec(010)` here is the strictness of
the enclosing code, and in a sloppy script that is sloppy.

The two `input_invalid_*` files pin the other side of the same rule: inside a class body the
enclosing mode *is* strict, so the identical parameter decorator is a syntax error — for the
leading-zero literal and for the legacy string escape alike. Both parsers reject them at
both goals.

## Why tsv Differs

Prettier's `typescript` parser rejects this input twice over. tsc's scanner has no sloppy
mode for numeric literals, so it raises

```
Octal literals are not allowed. Use the syntax '0o10'.
```

unconditionally — and tsc separately reports TS1206 (`Decorators are not valid here`) for a
parameter decorator outside a class method, a **checker**-side placement rule that tsv defers
to the diagnostics layer like the rest of its ambient/placement early errors (see
[checklist_typescript.md §Decorators](../../../../../docs/checklist_typescript.md)). Either
alone makes prettier useless as a formatting oracle here, so there is no `output_prettier.*`;
`prettier_rejects.txt` pins the first error the file draws and rule F6 live-verifies it.

**Acorn** accepts the whole file at `sourceType: 'script'` and rejects it at `'module'`, so
`expected.json` is the ordinary canonical AST and this is not a Svelte divergence — a Svelte
`<script>` is always a module, so nothing here is reachable from a `.svelte` file.

See [conformance_prettier_ts.md §Sloppy-script literals prettier refuses](../../../../../docs/conformance_prettier_ts.md#sloppy-script-literals-prettier-refuses),
and the frame's decision rules in
[conformance_prettier.md](../../../../../docs/conformance_prettier.md).
