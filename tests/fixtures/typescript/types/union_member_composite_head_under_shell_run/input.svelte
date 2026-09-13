<script lang="ts">
	// a union member whose paren shell carries a leading run over a SOLE-member composite:
	// the shell strips and the operator is dropped, so the shell's run and whatever the
	// author wrote after that operator are ONE run in the member's own gap - printed once,
	// at the member offset

	// a glued `//` over a `|`, the head gap holding a block
	type A =
		| // c1
		  /* d1 */ B
		| C;

	// the head gap holding a `//` of its own
	type D =
		| // c2
		  // d2
		  E
		| F;

	// a multi-line block in the shell, the head gap holding a `//`
	type G =
		| /* a3
b3 */
		  // d3
		  H
		| I;

	// the same block glued to the operator, so the run stays on one line
	type J =
		| /* a4
b4 */ /* d4 */ K
		| L;

	// nothing after the operator: the pair still does not survive
	type M =
		| // c5
		  N
		| O;

	// an `&` composite, transparent the same way
	type P =
		| // c6
		  /* d6 */ Q
		| R;

	// an object member
	type S =
		| // c7
		  /* d7 */ { a: 1 }
		| T;

	// a LATER member: the own-line half goes above the `| `, the rest after it
	type U =
		| V
		/* c8 */
		| /* d8 */ W;

	// control: the same shell with no composite inside, which lands on the same form
	type X =
		| // c9
		  Y
		| Z;
</script>
