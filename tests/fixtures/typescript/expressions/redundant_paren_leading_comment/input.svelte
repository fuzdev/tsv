<script lang="ts">
	// redundant grouping parens are stripped; the glued comment stays on the operand
	const a = /* c */ x;
	const b = /* c */ x + y;
	const c = /* c */ obj.prop;
</script>
