<script lang="ts">
	// A bare `yield` is a complete AssignmentExpression (ecma262 §13.15): no operator
	// may extend it (see input_invalid_*). Parenthesizing lifts it to a primary that
	// IS a valid operand, and an operator token that starts an expression is taken as
	// the yield *argument* rather than extending a bare yield.
	function* gen() {
		const a = (yield) || b;
		const c = (yield) ? x : y;
		yield a || b;
		yield +c;
		yield /a/g;
	}
</script>
