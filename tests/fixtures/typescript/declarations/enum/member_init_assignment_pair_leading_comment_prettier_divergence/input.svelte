<script lang="ts">
	// A comment the author wrote INSIDE an assignment initializer's clarity parens stays
	// inside them.
	enum E1 {
		A = (/* c */ a = b),
		B = 1
	}

	// a multi-line block comment likewise, and the value stays flat
	enum E2 {
		A = (/* c
		d */ a = b)
	}

	// null control: written outside the pair, both formatters keep it there
	enum E3 {
		A = /* c */ (a = b)
	}
</script>
