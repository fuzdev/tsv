<script lang="ts">
	// a deferred gap comment and a statement trailer reach one line end
	fn().bar as T; // c1
	// c2

	// the same past a following sibling in an argument list
	fn(fn1().bar, a); // c3
	// c4

	// a retained type shell's run, behind the member's own trailing comment
	type A =
		| (B & C) // c5
		// c6
		| D;

	// a block tail: the run separates at the body's own indent
	function fn2() {
		fn(); // c7
		// c8
	}
</script>
