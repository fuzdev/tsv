<script lang="ts">
	// The `...`→argument gap belongs to the `...`, so its run is emitted OUTSIDE any parens
	// the argument needs — one answer whether or not the argument's precedence requires a
	// pair. Both formatters agree; the sibling `await`→operand gap states the same rule.
	fn1(.../* c */ aaa);
	fn1(.../* c */ (bbb ?? ccc));

	// A comment the author gave its own line keeps that line, in both spellings.
	fn1(.../* c */ ddd);
	fn1(.../* c */ (eee ?? fff));

	// A `//` breaks, and the parens still follow the run rather than opening around it.
	fn1(
		...// c
		ggg
	);
	fn1(
		...// c
		(hhh ?? iii)
	);
</script>
