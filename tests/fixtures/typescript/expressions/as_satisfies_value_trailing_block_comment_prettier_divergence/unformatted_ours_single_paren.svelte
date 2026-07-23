<script lang="ts">
	// A block comment trailing the cast type. A cast is a value position, so the
	// trailing block defers past the statement `;` (line_suffix), matching the
	// declarator's own value-to-`;` trailing-comment handling.
	const a = x as (A /* t */);

	const b = x satisfies (B /* t */);

	// A block comment leads the cast type and another trails it: the leading block
	// trails the keyword inline, the trailing block defers past `;`.
	const c = x as (/* b */ C /* t */);

	const d = x satisfies (/* b */ D /* t */);
</script>
