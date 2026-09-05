<script>
	/* Two arguments is not a coercion - 100 chars, the chain stays inline */
	const a = Boolean(
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa || cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		other
	);

	/* 101 chars - the chain breaks with continuation indent */
	const b = Boolean(
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
			cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		other
	);

	/* Optional call is not a coercion - continuation indent */
	const c = Boolean?.(
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
			cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);

	/* new Boolean() is not a coercion - continuation indent */
	const d = new Boolean(
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
			cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);

	/* Coercion heading a member chain - no continuation indent */
	const e = Boolean(
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	).toString();
	const f = Boolean(
		cond_aaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
		cond_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	)?.toString();
</script>
