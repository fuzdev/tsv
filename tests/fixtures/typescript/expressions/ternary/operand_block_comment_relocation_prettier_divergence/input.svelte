<script>
	// Block comment after `?`: tsv keeps it after the operator, where it was written.
	const a = cond ? /* c1 */ x : y;

	// Block comment after `:`: same rule, the other slot.
	const b = cond ? x : /* c2 */ y;
</script>
