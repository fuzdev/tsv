<script lang="ts">
	// Declarator: the call fits on no line after `=`, and its argument list cannot hold the
	// mapped type flat, so the mapped type breaks and the call hugs the `=`
	const v1 = w.ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);

	// Exported declarator
	export const v2 = w.ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);

	// Assignment
	v3 = w.ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);

	// Class field
	class C {
		v4 = w.ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);
	}

	// Under an `await`
	const v5 = await w.ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);

	// No arguments
	const v6 = w.ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>();

	// At 100 the argument list holds the mapped type flat after the `=` break, so the `=` breaks
	// and the argument breaks out
	const v7 = fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);

	// At 101 the mapped type breaks, so the call hugs the `=`
	const v8 = ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);

	// At 100 the hugged call's first line fits
	const v9 = fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);

	// At 101 it does not: the `=` breaks and the mapped type breaks after it
	const v10 = fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);

	// Control: a `new` callee hugs the `=` in both formatters
	const v11 = new Wffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x);

	// Control: an object type argument is a break point in both formatters
	const v12 = w.ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ aaaa: 1; bbbb: 2 }>(x);

	// Control: a property value has no `=` to break after
	const o = {
		v13: w.ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff<{ readonly [K in X]: 1 }>(x)
	};
</script>
