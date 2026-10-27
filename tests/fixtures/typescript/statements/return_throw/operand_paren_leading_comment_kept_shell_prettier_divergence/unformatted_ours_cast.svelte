<script lang="ts">
	// A `return` / `throw` / `yield` argument whose LEFTMOST node is a cast's or a postfix
	// update's operand keeps that operand's shell: the pair holds the comment, so nothing
	// reaches the keyword's line and no hanging pair is added around it.
	function fn1() {
		return (
			// c
			a
		) satisfies B;
	}
	function fn2() {
		return (
			// c
			a
		)++;
	}
	function fn3() {
		return (
			// c
			a
		) as B;
	}
	function fn4() {
		throw (
			// c
			a
		) as B;
	}
	function* fn5() {
		yield (
			// c
			a
		) as B;
	}
	// a JSDoc cast in the shell of the cast's operand is the operand's own syntax, printed with
	// its own glue on the keyword's line
	function fn6() {
		return (/** @type {T} */
		(x)).b as U;
	}
</script>
