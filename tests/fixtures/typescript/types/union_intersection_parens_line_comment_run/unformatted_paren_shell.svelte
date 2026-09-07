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

	// A REQUIRED pair lifts its run to the same seam. A function type keeps its parens in
	// a member position, but prints them FLAT — both delimiters ride the inner type's own
	// lines — so the run still lands past the `)` on the member's own line
	type O = ((a: 1) => void // c35
	// c36
) | c;

	// a constructor type, the same
	type P = (new (a: 1) => void // c37
	// c38
) | c;

	// a conditional type, on a later member
	type Q = z | (a extends b ? c : d // c39
	// c40
) | c;

	// a nested intersection, whose required pair is that same flat wrapper
	type R = (a & b // c41
	// c42
) | c;

	// the INTERSECTION's answer for a required pair is the continuation line, as it is
	// for a stripped one
	type S = ((a: 1) => void // c43
	// c44
) & c;

	// the trailing-object intersection's aligned `})` closer is a UNION member's layout;
	// an intersection member levels from the `& ` line, so this pair is flat here too
	type T = (a & { x: X } // c45
	// c46
) & c;

	// the blank rules carry through a required pair: the union's run stays trailing its
	// member, so the blank survives...
	type U = ((a: 1) => void // c47

	// c48
) | c;

	// ...and the intersection's, which the member below re-reads as a leading run, drops
	type V = ((a: 1) => void // c49

	// c50
) & c;

	// control: a comment leading the member takes the whole run INSIDE the per-member
	// offset, required pair or not
	type W = z | /* c51 */ ((a: 1) => void // c52
	// c53
) | c;

	// A run LED BY A BLOCK hoists too — what the seam needs is a deferred BREAK to place,
	// and the `//` behind the block is what puts one there. The block trails the member
	// either way, so it travels with the run at no cost
	type X = (a /* c54 */
	// c55
) | c;

	// through a required pair, the same
	type Y = ((a: 1) => void /* c56 */
	// c57
) | c;

	type Z = (a & b /* c58 */
	// c59
) | c;

	// The INTERSECTION splits the same run instead of hoisting it whole: the `&` is emitted
	// between the member and the run, and a block renders where it is QUEUED, so the inline
	// prefix stays on the member's side of the operator and only the deferred tail travels
	type AA = (a /* c60 */
	// c61
) & c;

	// two leading blocks, the same
	type AB = (a /* c62 */ /* c63 */
	// c64
) & c;

	// a block PAST the first `//` rides the deferred tail — it is already behind a line
	// comment, so it defers with it rather than joining the prefix
	type AC = (a /* c65 */
	// c66
	/* c67 */
) & c;

	// the split keeps the run's own-line reading: a `//` the author glued to the member's
	// line still trails the `&`, where one written on its own line takes a line of its own
	type AD = (a /* c68 */ // c69
) & c;

	// The gap is the whole NEST, not the author's outermost layer: redundant layers strip
	// as a unit and print one pair between them, so a run sitting one layer in lifts to the
	// same seam
	type AE = (((a: 1) => void) // c70
	// c71
) | c;

	type AF = (((a: 1) => void) // c72
	// c73
) & c;

	// a CONSTRAINED infer is the fifth kind whose parens the member position requires - its
	// `extends` would otherwise absorb the `|`
	type AG = A extends ((infer U extends number) // c74
	// c75
) | c
		? 1
		: 2;

	// the pair need not print on ONE line for the run to lift past it: what the seam needs
	// is that the `)` rides the inner type's own last line, which it does however far the
	// inner breaks
	type AH = ((aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: 1, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: 2, cccccccccccccccccccccc: 3) => void // c76
	// c77
) | c;

	// four members: the seam is the FIRST one's, and every later boundary is untouched
	type AI = (a // c78
	// c79
) & b & c & d;
</script>
