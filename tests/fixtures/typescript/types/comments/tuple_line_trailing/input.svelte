<script lang="ts">
	// Line comment trailing first element
	type T = [
		A, // trailing
		B
	];

	// Line comment trailing multiple elements
	type U = [
		A, // trailing A
		B, // trailing B
		C
	];

	// Trailing on last element
	type V = [
		A,
		B // trailing
	];
</script>
