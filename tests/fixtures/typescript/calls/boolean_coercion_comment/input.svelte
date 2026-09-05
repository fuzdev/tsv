<script>
	/* Own-line leading comment forces the list open - the chain stays flat */
	const a = Boolean(
		// c
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);

	/* Inline block comment leads the argument - the chain stays flat */
	const b = Boolean(
		/* c */ cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);

	/* Trailing line comment on a chain head - the chain stays flat */
	const c = Boolean(
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb // c
	).toString();

	/* Own-line leading comment on a chain head - the chain stays flat */
	const d = Boolean(
		// c
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	).toString();
</script>
