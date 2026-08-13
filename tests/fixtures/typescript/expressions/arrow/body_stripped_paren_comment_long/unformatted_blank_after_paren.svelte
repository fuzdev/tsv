<script lang="ts">
	/* stripped grouping parens: the ternary's layout parens return, the comment inside them */
	const fn1 = (x) => (

		/* c */ cond ? a : b);

	/* two comments — only the one glued to the body token rides the body's own doc */
	const fn2 = (x) => (

		/* c1 */ /* c2 */ cond ? a : b);

	/* a ternary that breaks on its own — no layout parens, the comment leads the body */
	const fn3 = (x) =>
		/* c */ cond
			? `t1
t2`
			: b;

	/* curried typed chain — the innermost body is not a ternary */
	const fn4 =
		(x: T): U =>
		(y) =>
			/* c */ z;

	/* curried typed chain — the comment leads a nested arrow head */
	const fn5 =
		(x: T): U =>
		/* c */ (y) =>
			z;

	/* 100 chars — the body stays inline with its layout parens */
	const fn6_aaaaaaaaaaaaaaaaaaaaa = (x: string): string => (

		/* c */ cond ? aaaaaaaaaa : bbbbbbbbbb);

	/* 101 chars — the body breaks onto its own line and the parens drop */
	const fn7_aaaaaaaaaaaaaaaaaaaaaa = (x: string): string =>
		/* c */ cond ? aaaaaaaaaa : bbbbbbbbbb;
</script>
