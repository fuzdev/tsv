# default_prettier_ignore_head_prettier_divergence

A default value — a parameter default, a destructuring default, an array-pattern default, an
object-**shorthand** default — frozen by an own-line directive in its `=`→value gap. The slice
is the value's own node span, so the binding and the enclosing list stay parent-owned:

```ts
function fn(
	aaa =
		// prettier-ignore
		bbb  +  ccc
) {}
```

Prettier keeps the enclosing list **flat** around the frozen value, gluing the closer to the
value's last line (`function fn(aaa =⏎…⏎bbb  +  ccc) {}`, `const [iii =⏎…⏎jjj  ||  kkk] = lll`).
tsv breaks the list, as it does around every other frozen slice: the directive's own line is a
mandatory break inside the list, and a list that holds a break prints expanded.

The unprefixed hosts of the same head — a declarator initializer, an assignment RHS, an object
property value, a class field value — are the ordinary siblings
[init_prettier_ignore_head](../../../statements/variable/init_prettier_ignore_head/) and
[rhs_prettier_ignore_head](../rhs_prettier_ignore_head/), where prettier agrees.

## Reason

A frozen slice never changes how its container breaks — one rule at every freeze position, and
the same layout a plain own-line comment in that gap already produces; ◆design_choice. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
