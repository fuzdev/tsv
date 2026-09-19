<script lang="ts">
	// a `yield` expression is an AssignmentExpression too, and no operand either
	function* fn() {
		const a = x + yield b;
	}
</script>
