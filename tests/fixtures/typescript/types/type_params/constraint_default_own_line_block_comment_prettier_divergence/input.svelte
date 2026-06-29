<script lang="ts">
	// Own-line block comment after a type parameter's `extends` constraint: the
	// `<…>` expands and the constraint type hangs indented one level.
	type F<
		T extends
			/* a */
			U
	> = T;

	// A same-line block comment trailing `extends` (no preceding newline) with the
	// constraint type below stays trailing `extends`, type hanging.
	type G<
		T extends /* b */
			U
	> = T;

	// A same-line block comment glued to the constraint type stays fully inline.
	type H<T extends /* c */ U> = T;

	// Same three cases for a type parameter's `=` default.
	type I<
		T =
			/* d */
			U
	> = T;

	type J<
		T = /* e */
			U
	> = T;

	type K<T = /* f */ U> = T;
</script>
