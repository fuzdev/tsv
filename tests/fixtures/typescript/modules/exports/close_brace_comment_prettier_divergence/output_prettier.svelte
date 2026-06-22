<script>
	const a = 1,
		b = 2;

	// block comment between `}` and `;` stays after the brace
	export { a as x /* c */ };

	// line comments between `}` and `;` stay where the user placed them
	export {
		b as y // 1
		// 2
	};
</script>
