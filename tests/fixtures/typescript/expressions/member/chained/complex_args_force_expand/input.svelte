<script>
	// 3+ calls with non-simple regex arg (pattern > 5 chars) forces expansion
	const a = content
		.replace(/<[^>]+>/g, '')
		.replace(/\s+/g, ' ')
		.trim();

	// 3+ calls with simple args (all regex ≤ 5 chars, strings) stays flat
	const b = c.replace(/abc/g, 'x').replace(/de/g, 'y').trim();

	// 2 calls with non-simple regex — NOT forced (only 3+ calls triggers)
	const d = content.replace(/<[^>]+>/g, '').trim();

	// 3+ calls with arrow arg (non-simple) forces expansion
	const e = items
		.filter((x) => x.active)
		.map((x) => x.name)
		.sort();

	// 3+ calls with all simple args (identifiers, short strings) stays flat
	const f = arr.map(String).filter(Boolean).join(', ');
</script>
