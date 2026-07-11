<script lang="ts">
	// arrow-function operand of a non-null assertion keeps parens
	const r1 = ((a) => a)!;
	const r2 = ((a) => {
		a;
	})!;
</script>
