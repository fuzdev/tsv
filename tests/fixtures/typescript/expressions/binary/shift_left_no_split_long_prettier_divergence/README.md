# shift_left_no_split_long_prettier_divergence

The space tsv keeps between a list's `<` and a first type that opens with its own `<` — at the
positions where tsc never splits a `<<` token — counts toward the print width. A line the space brings to exactly 100 columns stays flat; one
it brings to 101 breaks after the `<`, where the type starts its own line and no space is
owed:

```ts
// tsv, 101 columns flat                // prettier, 100 columns glued
type A… = typeof f<                     type A… = typeof f<<T>(v: T) => R>;
	<T>(v: T) => R
>;
```

An assertion has one step more, prettier's own: over-width it first wraps its OPERAND
(`< <T>(v: T) => R>(⏎x⏎)`), the cast still flat and so still spaced, and breaks the cast only
when that does not fit either — one column later here.

## Why tsv differs

◆prettier_bug. Prettier's glued line is one column narrower, so each step arrives one column late —
and the flat line is the `<<` tsc rejects at an assertion, a `typeof` query and a heritage clause (see
[shift_left_type_assertion](../shift_left_type_assertion_prettier_divergence/)). From the
broken authoring prettier folds back to that flat line, so the over-width cases diverge from
either side (`unformatted_ours_flat` is the flat authoring).

## Reason

◆prettier_bug. See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(`<` `<` kept apart where tsc never splits a `<<`) and
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index).
