# signature_params_leading_line_comment_prettier_divergence

A line comment trailing the opening `(` of a **call** or **construct** signature
in a type literal/interface (`(// c`, `new (// c`) is preserved on the `(` line.
Prettier relocates it to its own line as the first parameter's leading comment.

tsv: keeps the comment trailing `(` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                  // prettier
type A = {              type A = {
	( // c                 (
		p: T,                  // c
	): void;                   p: T,
	new ( // c             ): void;
		p: T,              new (
	): void;                   // c
};                             p: T,
                           ): void;
                       };
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy) and preserves a comment trailing an opening delimiter in place
across the open-delimiter trailing-comment family (call `(`, object/array `{`/`[`,
function-type `(`, type-param `<`, …), via the shared
`Printer::delimiter_line_comment_prefix` helper. tsv applies that rule uniformly
to all signature kinds.

Prettier, by contrast, is **mixed** here: it keeps the comment trailing `(` for a
**method** signature (`m( // c`, which tsv therefore matches — see the regular
fixture
[method_signature_params_leading_line](../method_signature_params_leading_line/))
but relocates it onto its own line for **call** and **construct** signatures.
Those two are the divergence captured here.

An inline block comment (`(/* c */ p)`) and an own-line block comment are
unchanged and match Prettier; only a line comment trailing `(` diverges.

Before this, the comment swallowed the following tokens (`(// c p: T): void` —
invalid and non-idempotent); now it is preserved.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
