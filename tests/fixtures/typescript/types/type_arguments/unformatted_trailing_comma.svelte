<script lang="ts">
	// Basic type arguments
	type A = Promise<string,>;
	type B = Map<string, number,>;
	type C = Array<string | number,>;

	// Nested type arguments
	type D = Map<string, Promise<Array<number,>,>,>;
</script>
