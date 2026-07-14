<script lang="ts">
	// A line comment forces the member-only chain to break; the trailing non-null
	// `!` stays glued to its member (`.bar!`), never on its own line.
	const a = foo // c
		.bar!;

	// Optional member with a trailing non-null assertion (`?.bar!`).
	const b = foo // c
		?.bar!;

	// An own-line comment before the non-null member.
	const c = foo
		// c
		.bar!;
</script>
