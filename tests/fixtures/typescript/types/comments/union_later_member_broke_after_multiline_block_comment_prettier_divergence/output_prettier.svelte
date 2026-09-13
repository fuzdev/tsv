<script lang="ts">
	// A multi-line block the author broke after, glued after a later member's `|`: the
	// block leads its member and keeps the break after its `*/`, the member dropping to
	// the offset below it — as a first member's does. Prettier moves the block across
	// the `|` to trail the previous member.

	// a later member
	type A =
		| B /* c1
	d1 */
		| C
		| D;

	// the last member
	type E =
		| F
		| G /* c2
	d2 */
		| H;

	// an object member
	type I =
		| J /* c3
	d3 */
		| { a: 1 };

	// the indentable kind
	type K =
		| L /**
		 * c4
		 */
		| M;

	// a run: a single-line block glued ahead of the multi-line one
	type N =
		| O /* c5 */ /* c6
	d6 */
		| P;

	// control: the first member, prettier's own form
	type Q =
		| /* c7
	d7 */
		  R
		| S;

	// control: a multi-line block glued to the member keeps its line
	type T =
		| U
		| /* c8
	d8 */ V;
</script>
