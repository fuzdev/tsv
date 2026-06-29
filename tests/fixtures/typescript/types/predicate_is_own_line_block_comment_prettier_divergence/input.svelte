<script lang="ts">
	// Own-line block comment after a type predicate's `is`, predicate type on the
	// next line hanging indented one level.
	function f(x): x is
		/* a */
		T {
		return true;
	}

	// A same-line block comment trailing `is` (no preceding newline) with the
	// predicate type on the next line stays trailing `is`, type hanging.
	function g(x): x is /* b */
		U {
		return true;
	}

	// A same-line block comment glued to the type stays fully inline.
	function h(x): x is /* c */ V {
		return true;
	}
</script>
