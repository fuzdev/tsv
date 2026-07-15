# Divergence: import-equals `import`→`type` comment (preserve)

A block comment between `import` and the `type` keyword of an import-equals declaration
(`import /* c */ type C = require('./c');`). tsv keeps it after `import`; prettier **relocates** it
to the binding side of `type`.

```ts
// tsv (preserve)                         // prettier (relocate past `type`)
import /* c */ type C = require('./c');   import type /* c */ C = require('./c');
```

**Why tsv preserves:** the comment plausibly annotates the `import` itself; moving it past `type`
re-attaches it to the binding. This is the same rule prettier applies to a default import, where tsv
likewise preserves — [default_keyword_comment](../default_keyword_comment_prettier_divergence/).

This is the **only** import-equals header gap that diverges. The other four
(`import`→name, name→`=`, `=`→module-reference, `type`→name, and `export`→`import`) are preserved by
**both** formatters and live in the regular sibling
[equals_header_comment](../equals_header_comment/). tsv dropped all of them — plain content loss,
not a difference of opinion.

See [conformance_prettier.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
