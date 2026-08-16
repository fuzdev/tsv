<script lang="ts">
	// A run of two inside the true branch's shell stays together where it was written
	type A = T extends U ? (// c1
	// c2
	V) : W;

	// One comment in the shell, but the destination line is already taken
	type B = T extends U // c2
	? (// c1
	V) : W;

	// The false branch's shell answers both the same way
	type C = T extends U ? V : (// c1
	// c2
	W);
	type D = T extends U ? V // c2
	: (// c1
	W);

	// Control: a lone comment with a clear destination line still relocates to trail
	// the node before the operator
	type E = T extends U ? (// c
	V) : W;
</script>
