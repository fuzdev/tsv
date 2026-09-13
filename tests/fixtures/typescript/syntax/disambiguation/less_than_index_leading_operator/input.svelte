<script lang="ts">
	// a union whose comment breaks it into the leading-pipe layout is still a type
	// argument's index: a leading `|` or `&` can only open a type, never an array index
	const a1 = fn<
		A[
			| B // c1
			| C]
	>();
	const a2 = fn<
		A[
			| 0 // c2
			| 1]
	>();
	const a3 = fn<
		A[
			| -1 // c3
			| -2]
	>();
	const a4 = fn<
		A[(
			| B // c4
			| C
		)[]]
	>();

	// the authored leading operator normalizes away, through a paren shell too
	const b1 = fn<A[B | C]>();
	const b2 = fn<A[B & C]>();
	const b3 = fn<A[(B | C)[]]>();
	const b4 = fn<A[(B & C)[]]>();
</script>
