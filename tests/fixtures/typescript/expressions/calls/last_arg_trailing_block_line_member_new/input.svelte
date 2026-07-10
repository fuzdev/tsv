<script lang="ts">
	// Member-expression callee: the block trails the last argument, the line
	// comment trails after it (neither is dropped).
	obj.method(
		a,
		b /* c1 */ // c2
	);

	// Member chain: same on the final call's last argument.
	obj.method1().method2(
		a,
		b /* c1 */ // c2
	);

	// new expression: same.
	new C(
		a,
		b /* c1 */ // c2
	);
</script>
