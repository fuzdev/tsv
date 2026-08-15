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

	// Inside a CHAIN, with a numeric index — the one accessor kind the chain grouping
	// otherwise glues into the preceding group. The `//` makes it start a group
	// instead, and the bracket-interior rule above is unchanged by that.
	arr.foo() // c4
	[0];
</script>
