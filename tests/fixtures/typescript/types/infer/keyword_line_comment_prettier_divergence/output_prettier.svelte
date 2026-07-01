<script lang="ts">
	// A line comment trailing `infer`, the inferred name on the next line: the
	// comment stays trailing `infer` and the name hangs indented one level (the
	// shared keyword→value layout).
	type A = X extends infer // a
	R
		? R
		: never;

	// An own-line line comment keeps the comment on its own line, the name hanging
	// below it.
	type B = X extends infer // b
	R
		? R
		: never;
</script>
