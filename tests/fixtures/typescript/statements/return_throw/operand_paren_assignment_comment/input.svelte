<script lang="ts">
	// Trailing comment on a parenthesized return/throw assignment operand — must be
	// preserved (it was being dropped). return keeps a same-line block inside the
	// parens; throw trails it past the `;` (matching prettier in both cases).
	function f() {
		return (a = b /* c */);
	}
	function g() {
		throw (a = b); /* c */
	}
</script>
