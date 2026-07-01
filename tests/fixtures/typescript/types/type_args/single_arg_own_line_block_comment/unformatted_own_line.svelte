<script lang="ts">
	// A single type argument with an own-line block comment expands the `<…>` list
	// (both formatters); the compact authoring (unformatted_own_line) normalizes here.

	// type position
	type X = Foo<
	/* c */
	T>;

	// call position
	const x = f<
	/* c */
	T>(y);
</script>
