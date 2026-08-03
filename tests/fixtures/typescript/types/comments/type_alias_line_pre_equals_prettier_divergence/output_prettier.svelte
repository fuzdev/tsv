<script lang="ts">
	// line comment before = with type params stays before =
	type A<X> = B | C; // c

	// line comment before = without type params stays before =
	type D = E | F; // c
</script>
