# Divergence: line comments in the index-signature key-type→`]` gap

Line comments between an index signature's key type and its closing `]`
(`[key: string // b\n\t// d\n]`).

Prettier **never converges** on this input (rule F5, live-verified via
`prettier_nonconvergent.txt`): the own-line comment (`// d`) oscillates forever
between staying inside the brackets (`// d⏎]: number`) and being relocated
after `]` (`] // d⏎: number`, value `:` pushed to its own line), flipping on
every pass. tsv breaks the bracket and preserves each comment where the author
wrote it:

```ts
// prettier (oscillates between)             // tsv (preserves placement)
[                          [                 [
	key: string // b   ⇄     key: string // b    key: string // b
	// d                   ] // d                // d
]: number;                 : number;          ]: number;
```

A **same-line** trailing comment (`// b`) trails the type in both formatters —
that part matches; the un-settleable piece is the **own-line** comment (`// d`).
A block comment in this gap (`[key: string /* c */]`) stays inline in both and
is not a divergence. Same delimiter-gap comment class as every other open/close
delimiter (and the same oscillation as the sibling
`index_signature_bracket_colon_multi_comment_prettier_divergence`) — see
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy. Without the break, a line
comment here would swallow `]` and the value annotation (content loss).
