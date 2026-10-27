<script>
	// a block comment glued inside a discarded grouping pair on a left spine leads the
	// whole value, ahead of every pair the spine prints
	x = /*c*/ ((a ** 2) ** 2) ** 2;
	x = /*c*/ ((a + 2) * 2 * 2).b;
	x = /*c*/ (((a ? b : c) ** 2) ** 2) ** 2;
	x = /*c*/ (a + 2 + 2).b;
	x = /*c*/ (a.b, 1);
	x = [.../*c*/ ((a || d) && d)];
	f = async () => await /*c*/ ((a || b) && c);
	x = new /*c*/ ((a || d) && f).e();
	// except one the chain widens a member over, which stays mid-chain
	x = (a /*c*/.b
		.m() + 1)`t`;
	// a prefix unary prints the run its operand's spine holds inside its own pair
	x = !(/* d */ (a.b + 1).c);
	// a single-line block glued inside a binary's leftmost shell stays on the keyword's line
	function f() {
		return /* d */ a.b + 1;
	}
	// at the print width the line after it is glued to a space too, so the comment rides the chain's
	// line whether it fits (100 columns) or breaks inside the pair (101)
	function h() {
		return (
			/* d */ (a.b ** zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz) **
			q
		);
		return (
			/* d */ (a.b **
				zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz) **
			q
		);
	}
	// at `yield`, the claimed run takes the hanging pair where it keeps a line of its own, and
	// the binary's conditional pair where its line breaks only with the chain
	function* g() {
		yield /* d */

		(a.b ** zz) ** q;
		yield /* d */
		(a.b ** zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz) **
			q;
	}
</script>
