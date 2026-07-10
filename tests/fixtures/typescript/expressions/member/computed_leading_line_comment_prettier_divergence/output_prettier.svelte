<script lang="ts">
	// Line comment before the index: tsv keeps it inside the brackets, dropping the
	// index and `]` to their own lines so the `//` can't swallow them.
	const a =
		// c1
		arr[i];

	// Optional chain, same shape.
	const b =
		// c2
		obj?.[i];

	// Comment on the `[` line (not its own line): tsv keeps it trailing the `[`
	// (the open-delimiter-trailing rule), prettier relocates it.
	const c =
		arr // c3
		[i];
</script>
