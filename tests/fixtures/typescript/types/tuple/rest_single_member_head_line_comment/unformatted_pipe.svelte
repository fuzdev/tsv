<script lang="ts">
	// A `//` after a sole member's authored pipe in a rest element is the element gap's:
	// it stays glued to the `...`, the type dropped below it, exactly as the pipe-less
	// authoring prints. The pipe never survives
	type A = [...| // c1
	B[]];

	// after another element, and ahead of one
	type C = [D, ...| // c2
	E[]];

	type F = [...| // c3
	G[], H];

	// The block-run face of the same gap: a run the author GLUED — a multi-line block ahead
	// of a single-line one — and broke after. Both formatters drop the type below the run
	type I = [...|/* c4
	d4 */ /* c5 */
	J[]];
</script>
