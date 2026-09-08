<script lang="ts">
	// A `//` elsewhere in the conditional selects its breaking layout; a block ahead of a
	// branch union then answers from the seam exactly as the width-broken layout does.
	// Each cell is paired with its width-broken control.

	// glued multi-line block ahead of a hugging union: the hug keeps it glued
	type A1 = T extends U // c1
		? 1
		: /* c
		d */ { a: number } | null;
	type A2 =
		Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			? 1
			: /* c
		d */ { a: number } | null;

	// glued block ahead of a non-hug union that breaks: handed after the synthesized pipe
	type B1 = T extends U // c1
		? 1
		: | /* c */ Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;
	type B2 = T extends U // c1
		? | /* c */ Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: 1;
	type B3 =
		Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			? 1
			: | /* c */ Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
				| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// block glued to the operator with the author's break after it: the break is kept
	type C1 = T extends U // c1
		? 1
		: /* c */
			{ a: number } | null;
	type C2 =
		Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			? 1
			: /* c */
				{ a: number } | null;
</script>
