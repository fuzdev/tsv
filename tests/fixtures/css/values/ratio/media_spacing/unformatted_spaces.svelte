<style>
	/* a ratio's whitespace normalizes in a declaration value and is preserved in a
	media query: the media reader emits its `media-value` node verbatim */
	div {
		aspect-ratio:  16  /  9 ;
		aspect-ratio:  1.777 ;
		aspect-ratio:  auto  16  /  9 ;
	}

	@media (aspect-ratio: 16 / 9) {
		body {
			background: red;
		}
	}

	@media (aspect-ratio: 16  /  9) {
		body {
			background: red;
		}
	}
</style>
