<script>
	// Single blank line between properties
	const a = {
		property1: 'value1'

		, property2: 'value2'

		, property3: 'value3'
	};

	// Multiple blank lines collapse to one
	const b = {
		property1: 'value1'

		, property2: 'value2'
	};

	// Blank line only between some properties
	const c = {
		property1: 'value1'

		, property2: 'value2',
		property3: 'value3'

		, property4: 'value4'
	};

	// Mixed with methods
	const d = {
		property1: 'value1'

		, method1() {
			return 1;
		}

		, property2: 'value2'
	};

	// Short object with blank lines - wraps but preserves
	const e = {
		a: 1

		, b: 2
	};

	// Nested objects with blank lines
	const f = {
		outer: {
			prop1: 1

			, prop2: 2
		}

		, outer2: { prop3: 3 }
	};
</script>
