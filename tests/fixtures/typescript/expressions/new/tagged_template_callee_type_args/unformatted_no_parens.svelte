<script lang="ts">
	// Type arguments on a tagged-template `new` callee bind to the tag, not to
	// `new`: the whole `Foo<T>`x`` is the callee (a tagged template carrying its
	// type arguments), so `new` gets implicit empty arguments.
	const a = new Foo<T>`x`;
	const b = new Foo.Bar<T>`x`;

	// The tagged template is an ordinary MemberExpression, so the callee chain
	// continues past it: a trailing member access or a further tag still belongs
	// to the callee, not to the completed `new`.
	const c = new Foo<T>`x`.bar;
	const d = new Foo<T>`x``y`;

	// Contrast: with no trailing tag, the type arguments and call belong to `new`
	// itself.
	const e = new Foo<T>;

	// Explicit arguments before the tag: `new Foo<T>(1)` completes first, and the
	// tag applies to that call instead.
	const f = new Foo<T>(1)`x`;
</script>
