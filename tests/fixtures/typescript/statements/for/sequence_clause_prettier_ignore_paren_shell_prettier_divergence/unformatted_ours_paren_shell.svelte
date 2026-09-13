<script lang="ts">
	// An own-line directive inside the grouping shell the parser erases ahead of a for-header
	// sequence clause's FIRST operand freezes THAT operand; the `[~In]` pair the header needs
	// rides outside the slice.
	for ((
		// prettier-ignore
		'aaa'  in  bbb
	), g; ;) {
		fn();
	}

	// An operand with no `in` under it takes no pair.
	for ((
		// prettier-ignore
		aaa  +  bbb
	), g; ;) {
		fn();
	}

	// The update clause reads the same edge.
	for (; ; (
		// prettier-ignore
		aaa  +  bbb
	), g) {
		fn();
	}

	// A frozen LAST operand keeps its own shell gap: a block strips inline, outside the pair.
	for (fff,
		// prettier-ignore
		('aaa'  in  bbb /* t1 */); ;) {
		fn();
	}

	// A line comment in that gap retains the shell.
	for (fff,
		// prettier-ignore
		(aaa  +  bbb // t2
		); ;) {
		fn();
	}

	// A frozen operand that is itself a sequence carries the shell erased from ITS last
	// operand inside the slice, so the comment in it prints once, from the slice.
	for (fff,
		// prettier-ignore
		(bbb,  (ccc /* t3 */)); ;) {
		fn();
	}
</script>
