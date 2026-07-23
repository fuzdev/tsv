# embedded_language_verbatim_prettier_divergence

A tagged template's body is preserved **verbatim** by tsv — it does not *yet* reformat the
embedded language. Prettier reformats the embedded language it recognizes by tag name.

- **tsv**: `` html`<div>  {{label}}  </div>` `` and `` css`.a{color:red}` `` — kept exactly as authored
- **Prettier**: collapses the embedded HTML (`` html`<div>{{label}}</div>` ``) and expands the
  embedded CSS onto its own indented lines

`output_prettier.svelte` shows both bodies reformatted. tsv treats every tagged (and
decorator) template's quasi text as opaque source: `${…}` interpolations still format
normally, but the surrounding literal text is never re-indented, collapsed, or expanded.

## Reason

**Current, transitional behavior — not a permanent divergence.** tsv does not yet embed a
sub-formatter for the languages inside string templates, so it keeps every tagged template
body verbatim, which is the correct lossless interim stance (never reformat content it can't
yet format faithfully). Prettier's `embeddedLanguageFormatting` reformats bodies of tags it
recognizes (`html`, `css`, `graphql`, …); tsv keeps them as authored text **until embedded
support lands** (see below).

See [conformance_prettier.md](../../../../../../../docs/conformance_prettier.md) §TypeScript: Template Literals.

## Planned: embedded-language formatting

tsv will support formatting embedded bodies for the languages it already handles — CSS in
`` css`…` `` (through its CSS formatter) and Svelte/HTML markup in `` html`…` `` — the way
prettier's `embeddedLanguageFormatting` does, including prettier's comment-tagged form
(`` /* html */ `…` ``, `` /* css */ `…` ``). Scope is tsv's own languages (not GraphQL or
others). When it lands, this fixture flips: the `css` / `html` cases move from verbatim to
formatted expectations, and the residual divergence narrows to the languages tsv doesn't
format. Until then this fixture pins the current verbatim behavior.
