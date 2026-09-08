<script lang="ts">
	// A `//` after a sole member's authored pipe in a rest element is the element gap's:
	// it stays glued to the `...`, the type dropped below it, exactly as the pipe-less
	// authoring prints. The pipe never survives
	type A = [
		...// c1
		B[]
	];

	// after another element, and ahead of one
	type C = [
		D,
		...// c2
		E[]
	];

	type F = [
		...// c3
		G[],
		H
	];
</script>
