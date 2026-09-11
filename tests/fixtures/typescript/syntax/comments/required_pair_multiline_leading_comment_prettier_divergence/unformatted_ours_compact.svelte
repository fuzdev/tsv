<script lang="ts">
	// `as` operand
	const a1=(/* c
	d */ b+c+d) as T;

	// binary operand
	if((/* c
	d */ b+c+d)*2){fn();}

	// member-call base
	const a2=(/* c
	d */ b+c+d).toString();

	// non-null operand
	const a3=(/* c
	d */ b+c+d)!;

	// angle-bracket assertion operand
	const a4=<T>(/* c
	d */ b+c+d);

	// `await` operand
	async function fn1(){return await (/* c
		d */ b+c+d);}

	// null control: `typeof` prints its own pair, so prettier keeps the comment inside too
	const a5=typeof (/* c
	d */ b+c+d);

	// null control: `!` takes the same pair
	const a6=!(/* c
	d */ b+c+d);

	// null control: a sequence prints its own operand run, and the for header prints no pair
	for((/* c
	d */ b,c);;){fn();}

	// a sequence's own pair, which its printer emits rather than the position: the `as`
	// operand and a computed index each reach it through a different enclosing seam
	const s1=(/* c
	d */ b,c) as T;
	const s2=arr[(/* c
	d */ b,c)];

	// a two-comment RUN: only its LAST member is glued to the operand, so only that one is
	// owned — the pair's own emitter prints the first, and the two must partition the gap
	const r1=(/* w */ /* c
	d */ b+c+d) as T;

	// the same sequence with a `//` in the pair's trailing gap: that authoring routes to the
	// run builder's LINE-COMMENT twin, which owes the claim too — without it the twin's
	// pass breaks the operands and the next pass, with the `//` deferred past the `)`,
	// does not (an F1 wobble neither formatter's fixed point has)
	const s3=(/* c
	d */ b,c) as T; // t

	// null control: written outside the pair, both formatters keep it there
	const a7=/* c
	d */ (b+c+d) as T;

	export {a1,a2,a3,a4,a5,a6,a7, r1,s1,s2, s3,fn1};
</script>
