<script lang="ts">
	// a block comment sharing the `&`'s line stays after the operator, inline
	type A = X & /* a */ Y;

	// a block comment sharing the previous member's line stays before the operator
	type B = X /* b */ & Y;

	// blocks glued to each other are each adjacent to a neighbor — the run stays inline
	type C = X & /* c1 */ /* c2 */ Y;

	// object members keep their hug when the comment shares the `&`'s line
	type D = { a: 1 } & /* d */ { b: 2 };
</script>
