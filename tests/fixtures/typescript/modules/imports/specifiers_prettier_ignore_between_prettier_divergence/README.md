# specifiers_prettier_ignore_between_prettier_divergence

An own-line directive **between** two module specifiers freezes the **following**
specifier in tsv, like every other member list (Rule A):

```ts
import {
	aaa as a1,
	// prettier-ignore
	bbb   as   b1
} from './a';
```

Prettier reclassifies the directive: its module-specifier comment handler re-binds an
own-line comment whose preceding node is an `ImportSpecifier` / `ExportSpecifier` as
that specifier's **trailing** comment, so the freeze runs **backward** — the preceding
specifier is emitted verbatim and the following one reformats. On `input.svelte` the
preceding specifier is already normalized, so prettier's only visible act is
reformatting the frozen slice (`output_prettier.svelte`). `divergent_variant_backward`
makes the direction itself visible: with the *preceding* specifier perturbed instead,
prettier keeps it frozen (prettier-stable) while tsv normalizes it and freezes the
following specifier, landing on a third stable form.

The forward direction is the consistent reading of the list rule — tsv honors a
directive only where it **precedes** the node it names, the same reason a trailing
directive is permanently inert. The leading (`{`→first-specifier) gap is not affected:
prettier's handler needs a preceding specifier, so there both tools freeze forward and
the ordinary fixtures `imports/specifiers_prettier_ignore_member` and
`exports/specifiers_prettier_ignore_member` match.

Hosts covered: `import { … }` and `export { … } from`.

See [conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive).
