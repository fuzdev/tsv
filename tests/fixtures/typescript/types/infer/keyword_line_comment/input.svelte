<script lang="ts">
	// `infer` keyword followed by a line comment, the inferred name on the next line.
	type E = A extends infer // c
	R
		? R
		: never;
</script>
