<script lang="ts">
	// An optional element's operand needs its parens, so the author's shell and the
	// required pair are the same pair: a leading line comment renders inside that one
	type A = [(// c
	T | U)?];

	// A redundant double shell still collapses to the one pair
	type B = [((// c
	T | U))?];

	// The block spellings of both of the shell's gaps stay inline
	type C = [((/* c */ T | U))?];
	type D = [((T | U /* c */))?];

	// Control: with no comment the element prints exactly one pair
	type E = [((T | U))?];
</script>
