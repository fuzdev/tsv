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

	// a retained paren shell's trailing gap behaves the same way - and an extra redundant
	// LAYER changes nothing, the layers stripping as a unit so the retain question is the
	// whole nest's (unformatted_ours_paren_layer carries that authoring)
	type B1 = ((U // c4
	// c5
	))[];

	type B2 = ((U // c6
	))[];

	// a LEADING run in the same nest: the retain rule is asked of the whole nest, so the
	// pair survives and the enclosing gap must not claim the leading run out of it
	type B3 = ((// c7
	U // c8
	))[];
</script>
