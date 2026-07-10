<script lang="ts">
	// A mid-intersection object at the first transition (index 1) hugs the first member.
	// When it breaks internally its body indents one level and `}` returns to base,
	// with the trailing member on the `}` line - it does NOT gain the continuation indent.
	// 100 chars effective - object stays inline (at boundary)
	type Break100 = AAAAAAAAAAAAAAAAAAAA & { a: number; b: string; c: boolean } & CCCCCCCCCCCCCCCCCCC;

	// 101 chars effective - object breaks internally; body at base indent
	type Break101 = AAAAAAAAAAAAAAAAAAAA & {
		a: number;
		b: string;
		c: boolean;
	} & CCCCCCCCCCCCCCCCCCCC;

	// Consecutive objects: the second stays at base too (no `wasIndented` latch yet),
	// its body one level in, `}` back at base, tail member on the `}` line.
	type Cons = AAAAAAAAAA & { a: A } & {
		bbbbbbbbbb: B;
		cccccccccc: C;
		dddddddddd: D;
		eeeeeeeeee: E;
	} & FFFF;
</script>
