# value_whitespace_prettier_divergence

A `style:` directive value whose whitespace tsv cannot read as a pure separator: a run
*inside* a text part (`"'Font   Name', sans-serif"`, `"top   left"`), a run between value
parts the parts themselves may enclose in a CSS string (`"'{expr1}   {expr2}'"`), and the
newline run at the edge of a text part (`"translate({expr1},⏎      {expr2})"`). tsv copies
every one of them verbatim; prettier prints the value's text through its *fragment* text
printer, which collapses each whitespace run to a single `line`.

tsv (idempotent):

```svelte
<div style:font-family="'Font   Name', sans-serif"></div>
<div style:content="'{expr1}   {expr2}'"></div>
```

Prettier collapses, and inside a string that is a content change — `'Font   Name'` and
`'Font Name'` are two different font-family names, and a `content` string loses characters
it renders:

```svelte
<div style:font-family="'Font Name', sans-serif"></div>
<div style:content="'{expr1} {expr2}'"></div>
```

Worse where the attribute list breaks: the collapsed run is a `line`, so it renders as a
**line break** in that group, putting a raw newline inside the CSS string. A string token
cannot hold one (Syntax 3 §4.3.5 makes it a parse error → `<bad-string-token>`), so the
declaration the author wrote is silently dropped at runtime — see `output_prettier.svelte`.

The plugin's own policy is that attribute values are not formatted (`formattableAttributes`
is empty, "Prettier HTML does not format attributes at all"), and its raw-text guard names
the node type `Attribute` — a `StyleDirective` is a different type, so its value falls
through to the printer for element *content*. That is the whole mechanism, and it is why a
plain attribute in the identical shape keeps its text verbatim in both formatters.

The whitespace tsv *does* normalize here is the value part that is **nothing but**
whitespace, which the CSS grammar can only read as a token separator — the sibling
[multiline_value](../multiline_value/). tsv normalizes at part granularity and does not
reach inside a part that carries content. "Content" is read with the host's class
(`[ \t\n\r]`), so a `<NBSP>` and a **form feed** both hold their run out of the rule even
though CSS's own tokenizer would call the form feed a separator (§3.3 folds it to a
newline): every instrument here — `clean_nodes`, the render-key oracle, the blank-line
class `fabrication_audit` grades — treats a form feed as content, and tsv takes the
narrower of the two readings rather than respell a character they are all watching.

See [conformance_prettier_svelte.md §Svelte: Attributes](../../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).
