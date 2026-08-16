<script>
	// A blank line before a hole is the previous ELEMENT's blank, so it survives — the
	// comment slides past the hole's comma to lead the next element, the blank stays put
	const a = [
		1,
		2,

		,
		// c
		3
	];

	// The same blank with no comment in the gap
	const b = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,

		,
		cccccccccccccccccccccccccccccc
	];

	// A hole carries no blank of its own — a blank AFTER one is dropped
	const c = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		cccccccccccccccccccccccccccccc
	];
</script>
