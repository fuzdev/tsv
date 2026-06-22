<script>
	/* 100 chars arg line - stays inline */
	const a=fn((aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa+bbbbbbb)*100);

	/* 101 chars arg line - only outer binary breaks */
	const b=fn((aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa+bbbbbbb)*100);

	/* 106 chars arg line - breaks inside parens after + */
	const c=fn((aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa+bbbbbbbbbbb)*100);

	/* Division - same behavior */
	const d=fn((aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/bbbbbbbbbbb)*100);

	/* Modulo - same behavior */
	const e=fn((aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa%bbbbbbbbbbb)*100);

	/* Subtraction - same behavior */
	const f=fn((aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbb)*100);
</script>
