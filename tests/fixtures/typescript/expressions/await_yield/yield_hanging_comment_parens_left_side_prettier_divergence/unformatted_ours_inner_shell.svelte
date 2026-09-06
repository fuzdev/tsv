<script lang="ts">
	function* g() {
		// the comment sits inside the paren shell of the operand's LEFTMOST node — a member's
		// object, a binary's left operand, a conditional's test — rather than ahead of the
		// whole operand: the walk down the left side finds it, and the parens are retained
		// (`yield` is a restricted production: a newline after it triggers ASI)
		yield (
			// c
			a as any
		).b;
		yield (
			// c
			a as any
		) + b;
		yield (
			// c
			a as any
		) ? b : c;

		// the delegate form too
		yield* (
			// c
			a as any
		).b;
	}
</script>
