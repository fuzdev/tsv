<script lang="ts">
	// block comment before = with type params stays before =
	type A<X> /* c */ = B | C;

	// block comment before = without type params stays before =
	type D /* c */ = E | F;
</script>
