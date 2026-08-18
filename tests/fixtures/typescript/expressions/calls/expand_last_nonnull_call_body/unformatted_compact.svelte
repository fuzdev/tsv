<script lang="ts">
	// An arrow body that is a call through a trailing `!`: prettier's `couldExpandArg`
	// strips the non-null wrapper (`stripChainElementWrappers`), so the break-body
	// ladder applies — at all three spellings, not just the plain call and `new`.
	fn(aaaa, (x) => fn1(x, {
		a: b
	})!);

	new Fn(aaaa, (x) => fn1(x, {
		a: b
	})!);

	obj.mm(aaaa, (x) => fn1(x, {
		a: b
	})!);

	oo.aa.bb(aaaa, (x) => fn1(x, {
		a: b
	})!);

	// Repeated `!` strips the same way.
	obj.mm(aaaa, (x) => fn1(x, {
		a: b
	})!!);

	// Contrast: a MEMBER through `!` is not a call, so no spelling expands.
	obj.mm(aaaa, (x) => fn1(x, {
		a: b
	}).prop!);

	// Contrast: a ternary through `!` — the ternary arm reads the body's own type and
	// never strips, so the non-null wrapper takes this out of both arms.
	obj.mm(aaaa, (x) => (x ? fn1(x, {
		a: b
	}) : null)!);
</script>
