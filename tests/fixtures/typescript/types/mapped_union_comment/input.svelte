<script lang="ts">
	// Mapped type with union value and trailing comments - breaks after colon
	type A = {
		[K in T]?:
			| B<K> // comment1
			| null // comment2
			// comment3
			| ((x: B<K>) => B<K> | null);
	};

	// Mapped type with huggable union (object-like + null) and comment
	type C = {
		[K in T]?:
			| B<K> // comment
			| null;
	};

	// Mapped type without comments - stays inline
	type D = {[K in T]?: B<K> | null};
</script>
