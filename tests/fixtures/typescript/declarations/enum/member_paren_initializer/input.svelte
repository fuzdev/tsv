<script lang="ts">
	// a parenthesized sequence initializer keeps its parens, so the member spans through the `)`
	enum E1 {
		A = (a, b),
		B = 1
	}

	// the same on the last member, whose comma is never emitted
	enum E2 {
		A = 1,
		B = (a, b)
	}

	// the parens sit interior when an operator follows
	enum E3 {
		A = (a, b) + 1
	}

	// a comment inside the retained shell trails the member, past the printed parens
	enum E4 {
		A = (a, b) /* c */,
		B = 1
	}

	// a line comment in the same position defers the same way
	enum E5 {
		A = 1,
		B = (a, b) // c
	}
</script>
