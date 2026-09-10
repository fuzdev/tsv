# type_alias_line_pre_equals_hang_prettier_divergence

Line comment between a type alias head (name + optional type parameters) and the
`=`, where the value **breaks after `=`** and is *not* a union. The non-union face
of [type_alias_line_pre_equals_break](../type_alias_line_pre_equals_break_prettier_divergence/),
which covers the break-forced union; the inline-value face is
[type_alias_line_pre_equals](../type_alias_line_pre_equals_prettier_divergence/).

**tsv**: keeps the comment trailing the head (the uniform forced-continuation
indent, like every other before-`=` initializer site); the `=` drops to a
continuation line and the value's own lines sit at the **`=` level**:

```ts
type B<T> // c
	=
	Aaaaaaaaaaaaaaaaaaa<Xxxxxxxxxxxx> extends Bbbbbbbbbbbbbbbbbb<Yyyyyyyyyy>
		? Cccccccccccccccccccccc
		: Dddddddddddddddddddddd;
```

**Prettier**: relocates the head comment across the `=` and hangs the value below
it — reached non-idempotently, its first pass gluing the comment to the `=`
(`output_prettier.svelte`, unstable; the fixed point is pinned by
`audit_signature.txt`), its second dropping it to its own line above the value.
At both passes the value's lines land at the same column tsv puts them, so **the
divergence is the comment's position alone** — which is why the value's indent has
an oracle here even though its position does not.

Three arms break after `=` and all three are covered, one per emitter:

- **generic-conditional** (`B`) — prettier's `shouldBreakBeforeConditionalType`.
- **template-literal type** (`C`) — force-breaks, so it hangs whatever its width.
- **`fluid` with no reachable break point** (`D`) — the marker itself breaks. Its
  value is 95 chars: too wide for the `=` line, and exactly 100 one line down, so
  the fixture also pins that the extra level was costing a column of width.

`A`, `E`, `F` and `G` are null controls that must not move: the union already
hung at the `=` level, and an intersection, a plain conditional and a
complex-type-parameter head all **hug** the `=`, where a continuation one level in
is the correct shape. The hug itself is a separate, pre-existing divergence —
prettier hangs those values too — and this fixture makes no claim about it.

`unformatted_ours_double_indent.svelte` (every hanging value one level deeper —
what tsv used to emit) normalizes to input under tsv.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
