<script lang="ts">
	function* g() {
		// the comment sits inside the paren shell of the operand's LEFTMOST node — a member's
		// object, a binary's left operand, a conditional's test — rather than ahead of the
		// whole operand: the walk down the left side finds it, and the parens are retained
		// (`yield` is a restricted production: a newline after it triggers ASI)
		yield (
			// c
			(a as any).b
		);
		yield (
			// c
			(a as any) + b
		);
		yield (
			// c
			(a as any) ? b : c
		);

		// the delegate form too
		yield* (
			// c
			(a as any).b
		);

		// a block whose own text SPANS A LINE is the third spelling that forces it — a
		// MultiLineComment holding a line terminator IS one for ASI (ECMA-262 §12.4) — so the
		// left-side shell is load-bearing there too, glued to the leaf as it is
		yield (
			/* a
b */ a.b
		);
		yield (
			/* a
b */ a()
		);
		yield (
			/* a
b */ a!
		);

		// the binary operand is the one left-side kind `return` / `throw` reach through their
		// own conditional parens and `yield` does not, so only here does the walk carry it
		yield (
			/* a
b */ a + b
		);

		// a cast's operand keeps its own shell instead, one pair rather than two
		yield (
			/* a
b */ a
		) as B;

		// the delegate form takes the same answer
		yield* (
			/* a
b */ a.b
		);
	}
</script>
