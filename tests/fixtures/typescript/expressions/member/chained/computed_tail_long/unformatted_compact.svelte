<script lang="ts">
	// A computed lookup is GLUED to the lookup before it — prettier gives it no break point
	// of its own (`shouldInline` includes `node.computed`) — but it is not part of that
	// lookup's GROUP. The `.prop` group is measured only as far as the bracket softline, so
	// the brackets shed the width and every lookup stays on the first line.

	function fn(foo:A,cond:boolean,iii:number){
		// 108 flat: the BRACKETS break, the chain does not
		foo.bar.baz[
			cond?aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		];

		// a numeric index has no softline to stop the measure, so at 101 the last lookup
		// breaks instead and carries the index onto its own line
		foo.bar
			.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa[0];

		// and when both break, the glued brackets render at the LOOKUP's indent
		foo.bar
			.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa[
			iii
		];
	}
</script>
