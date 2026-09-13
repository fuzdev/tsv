<style>
	div {
		/* a head `-` the author spaced from a number or a hex keeps its gap inside a
		colour-adjuster function — prettier's own list of names, matched
		case-insensitively against the function the group belongs to */
		a: color(red rgb(- #900));
		b: rgb(- #900);
		c: RGB(- #900);
		d: blenda(- #900);
		e: rgb(- 20%);
		f: rgb(+ #900);
		/* the operator must open the comma group it sits in, so the rule is asked once
		per group rather than once per function */
		g: rgb(1, - #900);
		h: rgb(- #900 - #900);
		i: rgb(1 - #900);
		/* …and the bounds: the adjuster must be the group's OWN function, a nameless
		group inside one is not it, and an authored glue stays glued */
		j: color(-#900);
		k: f(-#900);
		l: rgb((-#900));
		m: rgb(-#900);
	}
</style>
