<script lang="ts">
	// a run at the end of an indexed access's brackets stays inside them, distinct,
	// one comment per line (unformatted_ours_flat carries the flat authoring)
	type A1 = T[
		K // c1
		// c2
	];

	// the same gap holding a single comment
	type A2 = T[
		K // c3
	];

	// a retained paren shell's trailing gap behaves the same way
	type B1 = (
		U // c4
		// c5
	)[];

	type B2 = (
		U // c6
	)[];
</script>
