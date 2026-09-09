<script lang="ts">
	// A DECORATED sole parameter declines the hug, so its object-type annotation
	// keeps a group of its own and stays flat while it fits.

	class Boundary {
		// The decorator's own line comes from the author
		authored(
			@dec
			a: {
				aaa: Aaaa;
			}
		) {}
		// ...and from the width, when the decorator and binding do not fit together
		width(
			@decoratorWithAQuiteLongNameIndeedXXXXXXXXXXXXXX(argument1, argument2, argument3)
			aaa: {
				aa: Aa;
			}
		) {}

		// A decorated PATTERN keeps its own group in both formatters
		pattern(
			@decoratorWithAQuiteLongNameIndeedXXXXXXXXXXXXXXXX(argument1, argument2, argument3)
			{ aaa }: Tt
		) {}
		// The other direction of the same dropped group: an annotation the AUTHOR expanded,
		// at a width where everything fits. Prettier collapses it (and with it the decorator's
		// own line); tsv keeps the expansion, whose break carries the decorator to its line.
		authoredObject(@dec a: { aaa: Aaaa }) {}
		// A second parameter takes the hug out of the question in both formatters
		two(
			@dec
			a: { aaa: Aaaa },
			b: Bbbb
		) {}
	}
</script>
