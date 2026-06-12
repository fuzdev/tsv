# param_default_line_comment_prettier_divergence

A line comment after a function parameter's `=` default, before the default
value (`function fn(p = // c\n\tv) {}`).

**tsv**: keeps the comment after `=`, with the value on the next line:

```
function fn(
	p = // c
	v,
) {}
```

**Prettier**: floats the comment out to trail the whole parameter, after the
value and its comma:

```
function fn(
	p = v, // c
) {}
```

Per Comment Position Philosophy: the user wrote the comment after `=` (before the
default value), so tsv keeps it associated with the default rather than floating
it past the value. Both forms are idempotent in their respective formatters. The
parameter's type-annotation union stays inline in both (the trailing comment is
zero-width, so it never forces the union to break). The same float applies to
arrow-function and method parameters. A same-line block comment
(`p = /* c */ v`) stays inline in both formatters and is not a divergence.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
