<script lang="ts">
	// The FIRST member's shell holds a leading `//`, so the run is hoisted above the
	// intersection and the member is rebuilt with its parens stripped. Its own lifted
	// TRAILING run is still only placeable at the boundary that FOLLOWS it, so the
	// boundary opens and the run lands on the continuation line — the answer every
	// other position already gave. A required pair, printed flat, is the shape that
	// reaches this route
	type A = // c1
		((a: 1) => void) & // c2
			// c3
			c;

	// a constructor type, the same
	type B = // c4
		(new (a: 1) => void) & // c5
			// c6
			c;

	// a conditional type
	type C = // c7
		(a extends b ? c : d) & // c8
			// c9
			c;

	// a nested intersection, whose required pair is that same flat wrapper
	type D = // c10
		(a & b) & // c11
			// c12
			c;

	// control: a BARE inner, whose shell run the enclosing `=` gap claims instead —
	// the same layout by a different route, which is what makes the two agree
	type E = // c13
		a & // c14
			// c15
			c;

	// A LATER member's shell leading run ends a line, so its boundary opens and
	// indents even where object-adjacency would otherwise hug — prettier's
	// `hasLeadingOwnLineComment(node)`, asked over the shell region the between-members
	// window cannot see
	type F = z &
		// c16
		{ x: X } & // c17
		// c18
		c;

	// the single-comment spelling of the same shell
	type G = z &
		// c19
		{ x: X } & c; // c20

	// and with an object on both sides of the boundary
	type H = { x: X } &
		// c21
		{ y: Y } & // c22
		// c23
		c;
	// the disjunct is about the LEADING run alone: a shell with no trailing run at all
	// opens its boundary just the same
	type I = z &
		// c24
		{ x: X } & c;

	// and a block-led run does not — a block ends no line, so nothing is relocated
	type J = z & /* c25 */ { x: X } & c;
	// Both rules are asked at the FORCED-MULTILINE loop too — an isolated comment in some
	// other gap routes there, and a boundary rule held at one loop only lets one authoring
	// reach two fixed points depending on what an unrelated gap happens to carry.
	// The first member's hold, and the hoisted run's own indent under that layout:
	type K = // c26
		((a: 1) => void) & // c27
			// c28
			c &
			// c29
			d;

	// and the leading-run disjunct, at a later member
	type L = y &
		// c30
		{ x: X } & // c31
		// c32
		c &
		// c33
		d;
	// The pair the member position REQUIRES stays closed under the hoist: the run is
	// already emitted above, so the pair may not open around it a second time
	// (`Printer::required_paren_open_run`'s shell arm takes the same claim filter its edge
	// arm has). With no trailing run to lift, this is the shape where nothing else would
	// have caught the double print
	type M = // c34
		(a & b) & c;

	type N = // c35
		(new () => C) & { a: 1 };
</script>
