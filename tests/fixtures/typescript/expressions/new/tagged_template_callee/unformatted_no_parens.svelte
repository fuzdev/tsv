<script>
	// A tagged template as a no-args `new` callee: the whole tagged template is
	// the callee (a greedy `new` consumes the trailing template before looking
	// for explicit arguments), so `new` gets implicit empty arguments.
	const a = new Foo`x`;
	const b = new Foo.Bar`x`;

	// Explicit parens around the tag are equivalent and get stripped.
	const c = new (Foo`x`);

	// Explicit arguments before the tag: `new Foo(1)` completes first, and the
	// tag applies to that call instead.
	const d = new Foo(1)`x`;
</script>
