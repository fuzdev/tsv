<script lang="ts">
	// A `//` after a sole member's authored operator at a tuple element is the tuple's own
	// gap, and takes that gap's answer exactly as the operator-less authoring does: on the
	// `[` line it keeps that line (the opening-delimiter rule)
	type A = [ // c1
		B
	];

	// after a comma it trails the comma
	type C = [
		D, // c2
		E
	];

	// a sole-member composite wrapping a real union: the redundant shell strips and the
	// union prints bare, the comment still on the `[` line
	type F = [ // c3
		G | H
	];
</script>
