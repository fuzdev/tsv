<script>
	// A numeric literal's decimal point is greedy, so an identifier can never abut it:
	// parenthesize or space the number to call a method or read a property.
	const a = (5).toString();
	const b = 1.5;
	const c = 1.5e3;
</script>
