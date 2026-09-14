<script lang="ts">
	// Own-line comment after `=` stays on its own line.
	type Default<
		U = // c
		V
	> = U;

	// Own-line comment after `extends` stays on its own line.
	type Constraint<
		T extends // c
		A
	> = T;

	// A glued run the author broke after, holding a multi-line block, hangs like the `//`.
	type GluedDefault<
		W /* c1
	d1 */ /* c2 */ = X
	> = W;

	// The `extends` constraint takes the same run the same way.
	type GluedConstraint<
		Y /* c3
	d3 */ /* c4 */ extends B
	> = Y;
</script>
