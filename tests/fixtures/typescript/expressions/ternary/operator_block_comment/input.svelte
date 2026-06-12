<script>
	// Block comment after ? — fits inline
	const a = b ? /* comment */ c : d;

	// Block comment after : — fits inline
	const e = b ? c : /* comment */ d;

	// Block comments after both operators
	const f = b ? /* c1 */ c : /* c2 */ d;

	// Short control — no comment
	const g = b ? c : d;
</script>
