<script lang="ts">
	// Union of object literals - content aligns with { position
	type T =
		| {
				prop1: A;
				prop2: B;
		  }
		| {
				prop3: C;
		  };
</script>
