<script>
	// inter-argument comment before a binary-chain last arg: the chain's
	// continuation operands indent one level under the first operand
	const a=fn(
		x,
		/* c */b1+
		// cont
		b2+b3,
	);

	// leading comment before a binary-chain first arg keeps the continuation indent
	const b=fn(
		/* c */b1+
		// cont
		b2+b3,y,
	);

	// ternary arg whose test is a logical chain keeps the continuation indent
	const c=fn(
		x,
		/* c */cond1 &&
		// cont
		cond2?p:q,
	);

	// trailing line comment after a binary-chain arg keeps continuation indent
	const d=fn(
		b1+
		// cont
		b2+b3,// trail
		y,
	);

	// assignment-expression arg keeps its clarity parens through the comment path
	const e=fn(/* c */(a=b),c);

	// a blank line between args forces expansion; a binary-chain arg (broken by its
	// own internal comment) keeps its continuation indent on that path too
	const f=fn(
		a,

		b1+
		// cont
		b2+b3,
	);
</script>
