<script lang="ts">
	// typed call after an optional link in the same chain
	a?.fn<T>();
	a?.fn<T>(b);
	a?.b.fn<T>(c);

	// optional typed call (the `?.` directly before the type args)
	fn?.<T>(a);
	a.fn?.<T>(b);

	// guards: a typed call with no optional link to its left keeps the plain shape,
	// whether standalone or continued by an optional chain afterward
	a.fn<T>(b);
	a.fn<T>()?.b;

	// optional typed call whose callee is itself a call — acorn-typescript marks the
	// callee CallExpression node optional too (the force-optional spine)
	g()?.<T>();
	a.b()?.<T>(c);
</script>
