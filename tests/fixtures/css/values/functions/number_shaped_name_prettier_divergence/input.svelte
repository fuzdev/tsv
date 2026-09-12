<style>
	a {
		/* postcss's `splitWord` reads a word matching its own NUMBER production as a
		numeric node, never a function name, and then fabricates a gap ahead of the group
		it synthesizes — tsv reads the whole word as the name and keeps the authoring */
		b: 50%(1.5);
		c: 1e5%(1.5);
		d: 1#(1.5);
		e: 1.5(1.5);
		/* the number is the name, so tsv emits the author's spelling of it */
		f: .5(1.5);
		g: 0a(1.5);
		/* controls: a word the number production does not match is a name on both sides,
		and an authored gap is a gap on both sides */
		h: a0(1.5);
		i: a.5(1.5);
		j: 1.5 (1.5);
	}
</style>
