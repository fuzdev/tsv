<script lang="ts">
	// A block comment glued to an enum member's `=` and broken after, above a multi-line
	// comment glued to the value: the first comment keeps the `=` line, and the value keeps
	// its own layout below the multi-line one.
	enum E {
		A = /* x */
		/* y
		 */ (
		aaa ? bbb : ccc),

		// a preserved multi-line comment: same layout
		B = /* x */
		/* y
z */ (
		bbb ? ccc : ddd)
	}
</script>
