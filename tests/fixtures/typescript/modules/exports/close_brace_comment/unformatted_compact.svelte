<script>
	const a = 1,
		b = 2;

	// block comment between `}` and `;` trails after the `;`
	export {a as x}/* c */;

	// line comments between `}` and `;` trail after the `;`
	export {b as y}// 1
	// 2
	;
</script>
