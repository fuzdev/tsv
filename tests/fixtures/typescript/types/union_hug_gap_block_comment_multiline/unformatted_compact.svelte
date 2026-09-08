<script lang="ts">
	// A MULTI-LINE block comment glued ahead of a should-hug union's first member binds
	// to the union, not the member — unlike a single-line block, which declines the hug —
	// so the union keeps hugging: the seam stays glued and the object member owns its
	// expansion on the comment's closing line. Inside the union's span, after an authored
	// leading pipe, a multi-line block is the member's like any other and declines.

	// Variable, alias, return type, function type, type argument
	let a: /* c
	d */ {aaaa: Aaaaaaaaa;bbb: Bbbbbb;cccccccccccccccccccc: Cccccccccccccccccccccccccc;dddd: D;eeee: E} | null;
	type B = /* c
	d */ {aaaa: Aaaaaaaaa;bbb: Bbbbbb;cccccccccccccccccccc: Cccccccccccccccccccccccccc;dddd: D;eeee: E} | null;
	function c(): /* c
	d */ {aaaa: Aaaaaaaaa;bbb: Bbbbbb;cccccccccccccccccccc: Cccccccccccccccccccccccccc;dddd: D;eeee: E} | null {}
	type L = () => /* c
	d */ {aaaa: Aaaaaaaaa;bbb: Bbbbbb;cccccccccccccccccccc: Cccccccccccccccccccccccccc;dddd: D;eeee: E}|null;
	type D = Map<
		string,
		/* c
	d */ {
			aaaa: Aaaaaaaaa;
			bbb: Bbbbbb;
			cccccccccccccccccccc: Cccccccccccccccccccccccccc;
			dddd: D;
			eeee: E;
		} | null
	>;

	// A JSDoc block, and a single-line block followed by a multi-line one
	let e: /**
	 * c
	 */ {aaaa: Aaaaaaaaa;bbb: Bbbbbb;cccccccccccccccccccc: Cccccccccccccccccccccccccc;dddd: D;eeee: E} | null;
	let f: /* c */ /* d
	e */ {aaaa: Aaaaaaaaa;bbb: Bbbbbb;cccccccccccccccccccc: Cccccccccccccccccccccccccc;dddd: D;eeee: E} | null;

	// The member fits on the comment's closing line
	let g: /* c
	d */ { aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccc } | null;

	// A union that does not hug keeps the comment ahead of its first pipe
	let h: /* c
	d */ Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb|Ccccccccccccccccccccccccc;

	// After an authored pipe the block is the member's and the hug is declined
	let i: | /* c
	d */ {aaaa: Aaaaaaaaa;bbb: Bbbbbb;cccccccccccccccccccc: Cccccccccccccccccccccccccc;dddd: D;eeee: E}|null;

	// A keyword seam behaves the same: a conditional branch, a type parameter bound
	type J = T extends U ? /* c
	d */ {aaaa: Aaaaaaaaa;bbb: Bbbbbb;cccccccccccccccccccc: Cccccccccccccccccccccccccc;dddd: D;eeee: E}|null : 2;
	function k<T extends /* c
	d */ {aaaa: Aaaaaaaaa;bbb: Bbbbbb;cccccccccccccccccccc: Cccccccccccccccccccccccccc;dddd: D;eeee: E}|null>() {}
</script>
