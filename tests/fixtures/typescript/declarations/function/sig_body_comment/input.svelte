<script>
	// function declaration: block comment between ) and body
	function fn(a) /* c */ {}

	// multiple params
	function fn2(a, b) /* c */ {}

	// function expression
	const expr = function (a) /* c */ {};

	// class method
	class A {
		m(a) /* c */ {}
	}

	// object method
	const obj = {
		m(a) /* c */ {},
	};
</script>
