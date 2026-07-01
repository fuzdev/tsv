<script lang="ts">
	// A single-line block comment in a type parameter's `extends` constraint or `=`
	// default gap collapses inline and keeps `<…>` collapsed; the own-line authoring
	// (unformatted_ours_own_line) collapses to this. Prettier relocates the comment
	// before the keyword (`T /* a */ extends U`).
	type H<
		T extends
			/* a */
			U
	> = T;

	type K<
		T =
			/* b */
			U
	> = T;
</script>
