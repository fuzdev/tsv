<script lang="ts">
	// Declarator: the call fits on no line after `=`, and its argument list cannot hold the
	// mapped type flat, so the mapped type breaks and the call hugs the `=`
	const v1 = w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);

	// Exported declarator
	export const v2 = w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);

	// Assignment, a callee rooted in a call with an argument
	v3 = w(1).fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);

	// Class field, a callee rooted in a `this` member call
	class C {
		v4 = this.w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
			readonly [K in X]: 1;
		}>(x);
	}

	// Under an `await`, a non-null root callee, a curried root call, and no arguments
	const v5 = await w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);
	const v6 = w!().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);
	const v7 = w()().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);
	const v8 = w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>();

	// At 100 the argument list holds the mapped type flat after the `=` break, so the `=` breaks
	// and the argument breaks out
	const v9 =
		w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{ readonly [K in X]: 1 }>(
			x
		);

	// At 101 the mapped type breaks, so the call hugs the `=`
	const v10 = w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);

	// At 100 the hugged call's first line fits
	const v11 = w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);

	// At 101 it does not: the `=` breaks and the mapped type breaks after it
	const v12 =
		w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
			readonly [K in X]: 1;
		}>(x);

	// At 100 the hugged first line of a call rooted in a call with an argument fits
	const v13 = w(1).fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);

	// At 101 it does not: the root call's argument breaks, so the call still hugs the `=`
	const v14 = w(
		1
	).fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);

	// Control: a member callee with no call in it hugs the `=` in both formatters
	const v15 = w.fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);

	// Control: a member-chain callee hugs the `=` in both formatters
	const v16 = w.a().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
		readonly [K in X]: 1;
	}>(x);

	// Control: a property value has no `=` to break after
	const o = {
		v17: w().fEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE<{
			readonly [K in X]: 1;
		}>(x)
	};
</script>
