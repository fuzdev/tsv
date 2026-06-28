<script lang="ts">
	// Line comment trailing an arg before its comma: relocates after the comma, no
	// spurious blank line before the next arg.
	foo(
		a // c
		,
		b
	);

	// Genuine blank line after the comma is still preserved.
	bar(
		a, // c

		b
	);
</script>
