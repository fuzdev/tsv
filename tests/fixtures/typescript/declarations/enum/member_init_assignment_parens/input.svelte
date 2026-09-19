<script lang="ts">
	// an assignment initializer takes clarity parens
	enum E1 {
		A = (a = b),
		B = 1
	}

	// the same on the last member, a compound operator, and a chained assignment
	enum E2 {
		A = (a += b),
		B = (a = b = c)
	}

	// a glued block comment stays in front of the parens
	enum E3 {
		A = /* c */ (a = b)
	}

	// a glued multi-line block comment does too, and the value stays flat
	enum E4 {
		A = /* c
		d */ (a = b)
	}

	// an indentable block comment likewise
	enum E5 {
		A = /**
		 * c
		 */ (a = b)
	}

	// a block comment inside the pair trails the member, past the printed parens
	enum E6 {
		A = (a = b) /* c */,
		B = 1
	}

	// a line comment in the same position defers past the comma
	enum E7 {
		A = (a = b), // c
		B = 1
	}
</script>
