<script lang="ts">
	// Line comment in an indexed access's `[`→index gap, kept where written with
	// the index type on the next line.
	type I = A[(// c
	K)];

	// A run the author GLUED — a multi-line block ahead of a single-line one — and then broke
	// after. The run's break is forced by the multi-line block, so the index drops below the
	// whole run, inside the brackets that own the break — the `=` keeps its hug, as it does
	// for every indexed access that breaks inside its own brackets.
	type J = A[(/* a
b */ /* c */
	K)];
</script>
