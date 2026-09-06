# supports_escaped_hash_separator_prettier_divergence

An at-rule prelude whose value is an ident carrying an **escaped `#`**, followed by another
token — `@supports (a: x\#FFF 5px) and (b: x\#FFF y)`.

The `\#` is a valid escape (CSS Syntax 3 §4.3.7) and §"Consume an ident sequence" takes it as
ident content, so `x\#FFF` is the single ident `x#FFF` — not an ident followed by a
hash-token — and the whitespace after it is the separator between two tokens.

Prettier reads the escape's payload as a `#`-token, which swallows what follows it into one
node, and its prelude pass then re-emits that node without the separator: `x\#FFF 5px` →
`x\#FFF5px` (one ident `x#FFF5px` where there were two tokens), `x\#FFF y` → `x\#FFFy`. The
space is content, so this is a token-stream change, not a spacing choice — and prettier's
form is its own fixed point, so a second pass raises nothing.

tsv steps the escape whole ahead of every prelude arm, so the ident ends where the author
ended it and the separator survives. The hash case on its own — where prettier gets it right
and tsv used to lowercase the escaped run as a hex colour — is the plain sibling
[supports_escaped_hash_token](../supports_escaped_hash_token/); the escaped-quote case is
[supports_escaped_quote](../supports_escaped_quote_prettier_divergence/).

See [conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules).
