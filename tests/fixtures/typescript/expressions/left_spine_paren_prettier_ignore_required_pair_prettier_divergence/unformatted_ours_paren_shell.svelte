<script lang="ts">
	// an argument-less `new` is the one operand whose SLICE needs a pair its node does not:
	// the printer supplies the `()` an unfrozen `new A` gets, and a slice cannot
	const a = (
		// prettier-ignore
		new A
	).b;

	// the same at a call, a non-null, a computed lookup and a template tag
	const c = (
		// prettier-ignore
		new D
	)();
	const e = (
		// prettier-ignore
		new F
	)!;
	const g = (
		// prettier-ignore
		new H
	)[0];
	const i = (
		// prettier-ignore
		new J
	)`t`;

	// a binary operand at a template tag
	const k = (
		// prettier-ignore
		l  +  m
	)`t`;

	// a ternary's test, whose `?:` is right-associative
	const n = (
		// prettier-ignore
		o  ?  p  :  q
	) ? r : s;

	// a `**` left operand, which is right-associative too
	const t = (
		// prettier-ignore
		u  **  v
	) ** 2;

	// one pair, not two, where the position's own leftmost target would also ask for one
	(
		// prettier-ignore
		{w:   1}
	).x;
	const y = () => (
		// prettier-ignore
		new Z
	).aa;

	// a `new` the author wrote with TYPE ARGUMENTS ends its slice at `>`, so the pair goes
	// around it wherever the tail's first token JOINS that `>`: bare, `new DD<T> + 1` reads
	// as `((new DD) < T) > +1`, and `[0]` rebinds through the same relational reading
	const ee = (
		// prettier-ignore
		new DD<T>
	) + 1;
	const ff = (
		// prettier-ignore
		new DD<T>
	)[0];

	// no join, so no pair — `**` and `?:` cannot open an expression after the `>`, the parse
	// backtracks to the type arguments, and the bare form is the same tree
	const gg = (
		// prettier-ignore
		new DD<T>
	) ** 2;
	const hh = (
		// prettier-ignore
		new DD<T>
	) ? ii : jj;

	// an instantiation the chain keeps as a parenthesized member object, at both lookups
	const kk = (
		// prettier-ignore
		ll<T>
	).nn;
	const oo = (
		// prettier-ignore
		ll<T>
	)[0];

	// under a `for` header's init the ambient `[~In]` pair rides outside the slice too
	for (
		(
			// prettier-ignore
			'bb'  in  cc
		) + 1;
		;
	) {}
</script>
