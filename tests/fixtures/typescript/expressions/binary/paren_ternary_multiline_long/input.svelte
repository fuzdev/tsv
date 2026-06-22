<script>
	// Simple: broken ternary left operand + binary (55 chars, well under print width)
	const a =
		(aaaaaa
			? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			: ccccccccccccccccccccccccccccccccccccccccc) + d;

	// Simple: 100 chars - broken ternary + binary stays on same line
	const b =
		(aaaaaa
			? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			: cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc) + d;

	// Simple: 101 chars - binary breaks after +
	const c =
		(aaaaaa
			? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			: ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc) +
		d;

	// Nullish + nested ternary: last line under print width (40 chars)
	const d2 = $derived(
		(a ??
			(b
				? b.fn1('xxxxxxxxxx')
					? b
					: `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx${fn2(b, 'x')}`
				: '')) + (c ? fn3(c, 'x') : '')
	);

	// Nullish + nested ternary: 100 chars - keeps + on same line
	const e = $derived(
		(a ??
			(b
				? b.fn1('xxxxxxxxxx')
					? b
					: `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx${fn2(b, 'x')}`
				: '')) + (c ? fn3(c, 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx') : '')
	);

	// Nullish + nested ternary: 101 chars - binary breaks after +
	const f = $derived(
		(a ??
			(b
				? b.fn1('xxxxxxxxxx')
					? b
					: `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx${fn2(b, 'x')}`
				: '')) + (c ? fn3(c, 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx') : '')
	);
</script>
