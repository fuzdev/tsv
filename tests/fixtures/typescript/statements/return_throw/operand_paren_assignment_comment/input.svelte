<script lang="ts">
	// Trailing block comment on a parenthesized assignment operand in return/throw
	// position. return keeps it inside the parens on the same line; throw trails it
	// after the `;`.
	function f() {
		return (a = b /* c */);
	}
	function g() {
		throw (a = b); /* c */
	}
</script>
