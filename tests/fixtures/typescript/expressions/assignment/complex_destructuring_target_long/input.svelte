<script>
	// A renamed property makes the target complex - it fits inline (100 chars)
	({ aaaa: aaaaaaaaaaaa, bbbbbbbbbbbb, cccccccccccc } = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx);

	// The same target one char wider - the target breaks, the value stays on the `=` line (101 chars)
	({
		aaaa: aaaaaaaaaaaa,
		bbbbbbbbbbbb,
		cccccccccccc
	} = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx);

	// A default counts the same as a rename
	({
		aaaaaaaaaaaa = 1,
		bbbbbbbbbbbb,
		cccccccccccc
	} = xxxxxxxxxxxxxxxxxxxxxxxxxxxx ?? yyyyyyyyyyyyyyyyyyyyyyyyyyyy);

	// Three shorthand properties are not a complex target - the value breaks after the operator
	({ aaaaaaaaaaaa, bbbbbbbbbbbb, cccccccccccc } =
		xxxxxxxxxxxxxxxxxxxxxxxxxxxx ?? yyyyyyyyyyyyyyyyyyyyyyyyyyyy);

	// Neither are two properties, renamed or not
	({ aaaa: aaaaaaaaaaaa, bbbbbbbbbbbb } =
		xxxxxxxxxxxxxxxxxxxxxxxxxxxx ?? yyyyyyyyyyyyyyyyyyyyyyyyyyyy);
</script>
