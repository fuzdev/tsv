<script lang="ts">
	// Two comments before `.baz`: the first trails `.bar`, the second on its own
	// line; the chain breaks at every member.
	const a =
		// c2
		foo
			.bar // c1
		.baz;

	// A single own-line comment before a member.
	const b =
		// c1
		foo.bar.baz;
</script>
