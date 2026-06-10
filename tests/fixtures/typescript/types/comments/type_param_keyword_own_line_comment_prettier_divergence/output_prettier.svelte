<script lang="ts">
	// Own-line comment after `=` stays on its own line.
	type Default<
		U = // c
		V,
	> = U;

	// Own-line comment after `extends` stays on its own line.
	type Constraint<
		T extends // c
		A,
	> = T;
</script>
