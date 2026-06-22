<script>
	// Leading block comment (single arg, inline)
	obj.method1().method2(/* comment */ value);

	// Leading block comment (multiple args, inline)
	obj.method1().method2(/* comment */ a, b);

	// Leading block comment with optional chaining
	obj?.method1()?.method2(/* comment */ value);

	// Leading block comment with expanded trailing comment
	obj.method1().method2(
		/* leading */ value // trailing
	);
</script>
