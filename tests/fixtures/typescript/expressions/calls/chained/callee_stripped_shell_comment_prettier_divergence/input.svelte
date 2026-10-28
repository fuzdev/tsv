<script lang="ts">
	// A comment in grouping parens the parser strips from around a call's callee, or a
	// `!`'s operand, leads the chain, whether or not a shell sits inside that one.
	/* c1 */ x.y.p();
	/* c2 */ x.y.p().t();
	/* c3 */ x.y.p!();
	/* c4 */ x.y.p!.t();
	/* c5 */ (x?.y).s[0]();
	// c6
	x.y.p();
	/* c7 */ /* c8 */ /* c9 */ x.y.p().q();
	/* c10 */ x.y.p();
</script>
