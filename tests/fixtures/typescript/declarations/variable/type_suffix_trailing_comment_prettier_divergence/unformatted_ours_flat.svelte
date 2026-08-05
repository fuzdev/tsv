<script lang="ts">
	// a line comment at the end of an indexed access's brackets stays inside them,
	// like every other bracketed type region, with the `]` on its own line
	// (unformatted_ours_flat carries the flat authoring)
	const a1: T[K // c1
	] = y;

	// a redundant paren shell is retained for the same reason, as in value position
	const a2: (A // c2
	) = y;

	// with no initializer there is no `=` to reach either
	let b: T[K // c3
	];
</script>
