<script lang="ts">
	// The leftmost node of a `return` / `throw` / `yield` argument sits in a pair the printer
	// REQUIRES (`(a as any).b`, `(a++).b`), so the multi-line block glued inside it can never
	// reach the keyword's line: the pair holds it. The left-side walk that adds a hanging pair
	// for a STRIPPED shell
	// (operand_paren_leading_multiline_block_left_side_prettier_divergence) stops here —
	// descending would wrap a second pair around the one that already holds the run.
	function fn1() {
		return (/* a
b */ a as any).b;
	}
	function fn2() {
		return (/* a
b */ a++).b;
	}
	function* fn3() {
		yield (/* a
b */ a as any).b;
	}
</script>
