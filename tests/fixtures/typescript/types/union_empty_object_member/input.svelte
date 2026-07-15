<script lang="ts">
	// Empty object type as a union member keeps its braces tight (`{}`, no
	// bracketSpacing) — an empty object literal has no content to space around.
	type Bare = A | {};

	// Leading empty union member behaves the same.
	type Leading = {} | A;

	// Empty object inside a parenthesized intersection union member.
	type Paren = A | (B & {});

	// An interior comment in an empty union-member object is preserved.
	type Commented = A | { /* c */ };
</script>
