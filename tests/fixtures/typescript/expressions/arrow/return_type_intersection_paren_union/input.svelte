<script lang="ts">
	// Parenthesized union (object-type member) inside an intersection return type
	const f1 = (): A & ({ b: B } | C) => x;

	// Parenthesized union (tuple-type member) inside an intersection return type
	const f2 = (): A & ([B] | C) => x;

	// Parenthesized union (identifier members) inside an intersection return type
	const f3 = (): A & (B | C) => x;

	// Object types on both sides of the intersection
	const f4 = (): { a: A } & ({ b: B } | { c: C }) => x;
</script>
