<script lang="ts">
	const chain_no_newline = `${object.propertyAAAAAAAAAAAAAAAAAAAAA.propertyBBBBBBBBBBBBBBBBBBB.propertyCCCCCCCCCCCCCCCC}`;
	const chain_newline = `${object.propertyAAAAAAAAAAAAAAAAAAAAA.
propertyBBBBBBBBBBBBBBBBBBB.propertyCCCCCCCCCCCCCCCC}`;

	const binary_no_newline = `${leftOperandAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA && rightOperandBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB}`;
	const binary_newline = `${leftOperandAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA &&
rightOperandBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB}`;

	const ternary_no_newline = `${conditionAAAAAAAAAAAAAAAAAAAAA ? consequentBBBBBBBBBBBBBBBBBB : alternateCCCCCCCCCCCCCCCCCC}`;
	const ternary_newline = `${conditionAAAAAAAAAAAAAAAAAAAAA ?
consequentBBBBBBBBBBBBBBBBBB : alternateCCCCCCCCCCCCCCCCCC}`;

	const newline_but_fits = `${object.
shortProperty}`;
</script>
