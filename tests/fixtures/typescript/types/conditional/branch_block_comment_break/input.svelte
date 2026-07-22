<script lang="ts">
	// Broken conditional type: a block comment after `:` keeps its authored break
	type A = Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? Baaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		: /* c */
			Caaaaaaaaaaaaaaaa;

	// Same on the `?` branch
	type B = Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? /* c */
			Baaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		: Caaaaaaaaaaaaaaaa;

	// Glued control: a comment the author kept on the branch line stays glued
	type C = Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? Baaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		: /* c */ Caaaaaaaaaaaaaaaa;

	// A glued multiline block keeps its value glued on the closing line
	type D = Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? Baaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		: /* l1
			 l2 */ Caaaaaaaaaaaaaaaa;

	// Breaking-union branch: the comment stays on the `:` line, members break below
	type E = Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? Baaaa
		: /* c */
			| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			| Ccccccccccccccccccccccccccccccccccccccccccc;

	// Chained: a comment on the middle branch keeps its break
	type F = Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? Baaaa
		: /* c */
			Zaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Waaaaaaaaaaaaaaaaaaaaaaaaaaa
			? Daaaa
			: Eaaaa;

	// A blank line after the comment is preserved along with the break
	type G = Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends Yaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? Baaaa
		: /* c */

			Caaaaaaaaaaaaaaaa;
</script>
