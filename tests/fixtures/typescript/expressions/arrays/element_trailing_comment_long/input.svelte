<script lang="ts">
	// Array elements with trailing line comments at exactly 101 chars
	// Trailing comments excluded from width - elements stay compact

	// Object element (101 chars)
	const a = [
		{ a: 'val00001', b: 'val00002', c: 'val00003', d: 'val00004', e: 'val5' } // aaaaaaaaaaaaaaaaaaaaa
	];

	// Multiple object elements (101 chars each)
	const b = [
		{ a: 'val00001', b: 'val00002', c: 'val00003', d: 'val00004', e: 'val5' }, // aaaaaaaaaaaaaaaaaaaaa
		{ a: 'val00001', b: 'val00002', c: 'val00003', d: 'val00004', e: 'val5' } // aaaaaaaaaaaaaaaaaaaaa
	];

	// Array element (101 chars)
	const c = [
		[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21] // aaaaaaaaaaaaaaaaa
	];

	// Spread element (101 chars)
	const d = [
		...arr0000000000000000000000000000000000000000000000000001 // aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	];

	// Call element (101 chars)
	const e = [
		fn(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb) // aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	];

	// Short elements under 100 chars (baseline)
	const f = [
		{ a: 1 }, // aaa
		[1, 2] // aaa
	];
</script>
