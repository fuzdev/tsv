<script lang="ts">
	// A single-line block comment after `infer` collapses inline; the own-line
	// authoring (unformatted_ours_own_line) collapses to this. Prettier reaches the
	// same inline form but is non-idempotent (its first pass leaves the name on the
	// next line).
	type A = X extends infer /* a */ R ? R : never;

	// With an `extends` constraint on the inferred name.
	type C = X extends infer /* c */ U extends string ? U : never;
</script>
