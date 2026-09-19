<script lang="ts">
	// A line comment trailing an enum member's `=` keeps the `=` line, and the value hangs
	// below it, as at a declarator and a class field.
	enum E1 {
		A = a, // c
		B = 1
	}

	// a second line comment trailing the member stays a separate comment
	enum E2 {
		A = a, // c1 // c2
		B = 1
	}

	// the same for a binary value and for an assignment, which keeps its clarity parens
	enum E3 {
		A = a ?? b, // c
		B = (a = // c
			b)
	}
</script>
