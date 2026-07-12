<script lang="ts">
	// A parenthesized arrow is a complete primary and CAN be an operand; a bare
	// block-bodied arrow is a complete AssignmentExpression and cannot be extended
	// by any operator (binary, `as`, ternary, assignment target — see input_invalid_*).
	const a = (() => {}) || b;
	const c = (() => {}) as T;
</script>
