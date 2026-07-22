<script lang="ts">
	// An empty object type as a union member keeps its braces tight (`{}`, no
	// bracketSpacing) — an empty object has no content to space around, so both
	// formatters agree.
	type Bare = A | {};

	// Leading empty union member behaves the same.
	type Leading = {} | A;

	// Empty object inside a parenthesized intersection union member.
	type Paren = A | (B & {});
</script>
