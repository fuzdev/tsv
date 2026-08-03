<script lang="ts">
	// line comment before = with type params stays before =
	type A<X>
		// c
		= B | C;

	// line comment before = without type params stays before =
	type D
		// c
		= E | F;
</script>
