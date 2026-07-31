<script lang="ts">
	// A line comment on its own line before `?` keeps that line.
	type A = B extends C
		// c
		? D
		: E;

	// Consecutive own-line comments each keep their line — merging them would turn
	// the second `//` into text of the first.
	type F = B extends C
		// c1
		// c2
		? D
		: E;

	// The false-branch sibling: an own-line run before `:` keeps its lines too.
	type H = B extends C
		? D
		// c1
		// c2
		: E;

	// Control: a comment the author left on the extends-type's line trails it.
	type G = B extends C // c
		? D
		: E;
</script>
