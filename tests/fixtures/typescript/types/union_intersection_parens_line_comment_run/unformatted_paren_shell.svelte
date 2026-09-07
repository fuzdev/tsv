<script lang="ts">
	// A run lifted out of a stripped UNION member's paren shell prints at the member
	// seam, flush under the `|`, not at the member's own two-column offset
	type A = (a // c1
	// c2
) | c;

	// a later member's shell takes the same seam
	type B = z | (a // c3
	// c4
) | c;

	// an own-line comment alone in the shell, nothing on the member's own line
	type C = (a
	// c5
) | c;

	// the INTERSECTION's answer to the same question: the run opens the continuation
	// line the `&` breaks onto, so it takes that line's indent
	type D = (a // c6
	// c7
) & c;

	// and the same with a third member past the boundary
	type E = (a // c8
	// c9
) & b & c;

	// a comment in a LATER member's gap forces the multiline intersection layout; the
	// first member's lifted run still lands on the continuation
	type F = (a // c10
	// c11
) & b & // c12
	c;

	// an author blank inside the run: the UNION's run stays trailing its member, so the
	// blank survives
	type G = (a // c13

	// c14
) | c;

	// the INTERSECTION's does not - the comment attaches to the member below and the `&`
	// has been pulled onto the line above, so the blank has nowhere to land and both
	// formatters drop it
	type H = (a // c15

	// c16
) & c;

	// THE OTHER ARM: a member carrying LEADING comments takes its whole run INSIDE the
	// per-member offset, so the same run indents two columns instead of sitting flush.
	// In the shell...
	type I = (/* c17 */ a // c18
	// c19
) | c;

	// ...in the union's own gap, which is where the reparse reads both of them...
	type J = /* c20 */ (a // c21
	// c22
) | c;

	// ...and on a later member, whose region opens past its own `|`
	type K = z | (/* c23 */ a // c24
	// c25
) | c;

	// control: a comment before the `|` trails the member ABOVE, so it decides nothing
	// about this one and the run stays flush
	type L = z /* c26 */ | (a // c27
	// c28
) | c;

	// ...and the split is asked of the OUTPUT, not of the source bytes: an own-line gap
	// comment is relocated ABOVE the `|`, where the reparse reads it as `z`'s trailing
	// run, so it leads nothing and the run below stays flush
	type M = z |

/* c29 */ (a // c30
	// c31
) | c;

	// the FIRST member has no earlier member to be relocated onto, so its whole gap run
	// leads it however it was written - and the run below indents
	type N =
		|

/* c32 */ (a // c33
	// c34
) | c;
</script>
