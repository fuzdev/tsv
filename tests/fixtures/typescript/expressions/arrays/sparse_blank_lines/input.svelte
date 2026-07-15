<script>
	// Hole in a breaking array - the hole's own line is structure, not a blank line
	const a = [
		'aaaaaaaaaaaaaaaa',
		,
		'bbbbbbbbbbbbbbbb',
		'cccccccccccccccc',
		'dddddddddddddddd',
		'eeeeeeee'
	];

	// Hole in a fitting array - stays inline
	const b = [1, , 2];

	// Blank line before a hole - preserved
	const c = [
		'aaaaaaaaaaaaaaaa',

		,
		'bbbbbbbbbbbbbbbb',
		'cccccccccccccccc',
		'dddddddddddddddd',
		'eeeeeeee'
	];

	// Trailing hole - mandatory comma, no blank line before it
	const d = [
		'aaaaaaaaaaaaaaaa',
		'bbbbbbbbbbbbbbbb',
		'cccccccccccccccc',
		'dddddddddddddddd',
		'eeeeeeeeee',
		,
	];

	// Consecutive holes
	const e = [
		'aaaaaaaaaaaaaaaa',
		,
		,
		'bbbbbbbbbbbbbbbb',
		'cccccccccccccccc',
		'dddddddddddddddd',
		'eeeeeeee'
	];

	// Multiline template forces expansion - hole gains no blank line
	const f = [
		`line1
line2`,
		,
		'b'
	];

	// Blank line before a hole - preserved when a line comment expands the array
	const g = [
		'aaaaaaaaaaaaaaaa', // c1

		,
		'bbbbbbbbbbbbbbbb'
	];

	// Line comment expands the array - hole gains no blank line
	const h = [
		'aaaaaaaaaaaaaaaa', // c2
		,
		'bbbbbbbbbbbbbbbb'
	];
</script>
