<script lang="ts">
	// A member-only chain (no calls) with line comments between members: tsv breaks
	// the chain and keeps each comment where the author wrote it. Prettier instead
	// hoists the own-line comment before the whole expression and trails the rest,
	// merging consecutive line comments. tsv preserves position (no merge/reverse).

	// Two comments before `.baz`: the first trails `.bar`, the second gets its own
	// line. The chain breaks at every member (as it does once a call forces a break).
	const a = foo
		.bar // c1
		// c2
		.baz;

	// A single own-line comment before a member.
	const b = foo
		.bar
		// c1
		.baz;
</script>
