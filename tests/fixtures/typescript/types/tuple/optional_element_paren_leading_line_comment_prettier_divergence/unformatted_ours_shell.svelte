<script lang="ts">
	// Conditional operand
	type A = [(
	// c
	B extends C ? D : E)?];

	// Function-type operand
	type F = [(
	// c
	() => void)?];

	// Intersection operand
	type G = [(
	// c
	H & I)?];

	// A redundant double shell still collapses to the one pair
	type J = [((
	// c
	K extends L ? M : N))?];

	// Control: the UNION operand, where prettier already renders the run inside the
	// same pair — the parting is prettier's own inconsistency, not a tsv preference
	type O = [(
	// c
	P | Q)?];
</script>
