<script lang="ts">
	// A line comment between a named tuple member's label and its `:` stays after
	// the label; the `: type` continuation indents one level.
	type A = [
		b // c
			: string
	];

	// An optional member's `?` marker takes the same continuation.
	type B = [
		b? // c
			: string
	];

	// So does a rest member's label.
	type C = [
		...r // c
			: string[]
	];

	// A stacked run stays with the label; the tail continues below the run.
	type D = [
		b // c1
			// c2
			: string
	];
</script>
