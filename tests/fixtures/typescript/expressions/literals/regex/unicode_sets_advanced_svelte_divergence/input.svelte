<script lang="ts">
	// Set subtraction (-- operator)
	const a = /[a-z--[aeiou]]/v;

	// Set intersection (&& operator)
	const b = /[\p{Letter}&&\p{ASCII}]/v;

	// Nested character classes
	const c = /[[0-9][a-f]]/v;

	// Complex: nested with operations
	const d = /[[\p{Letter}--\p{ASCII}]&&\p{Script=Latin}]/v;

	// String literals in character classes (\q{})
	const e = /[\q{abc|def}]/v;
</script>
