<script lang="ts">
	// the shell ends an intersection inside a union member: the comment trails
	// the whole member and the intersection prints flat
	type A1 =
		| (B &
				A) // c1
		| C;

	// same through a redundant outer paren layer around the intersection
	type A2 =
		| A
		| (B &
				C) // c2
		| D;

	// two composites deep: every intermediate composite prints flat
	type A3 =
		| A
		| (B &
				(C &
					D)) // c3
		| E;
</script>
