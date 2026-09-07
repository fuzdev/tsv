<style>
	div {
		/* the wrap decision reads the value the printer will EMIT, so a commented
		function whose normalized form is exactly 100 columns stays on one line */
		background-image: linear-gradient(#fff /* comment */, #aaa 1.5px, #bbbaaaaaaaaaaaaaaaaaaaaaaaa);
		/* one column more and it breaks one argument per line */
		background-image: linear-gradient(
			#fff /* comment */,
			#aaa 1.5px,
			#bbbaaaaaaaaaaaaaaaaaaaaaaaaa
		);
	}
</style>
