<script lang="ts">
	//
	// Function Expression Body - Blank Lines Force Chain Breaks
	//
	// Blank lines in chains force all members to break, regardless of fit.
	// This applies in all contexts: arrow functions, IIFEs, function
	// declarations, class methods, blocks, and top-level.

	// Single-char identifier - blank line forces break
	fn(() => {
		const b = a
			.method1()

			.method2();
	});

	// Two-char identifier - blank line forces break
	fn(() => {
		const b = ab
			.method1()

			.method2();
	});

	// Three-char identifier - blank line forces break
	fn(() => {
		const b = abc
			.method1()

			.method2();
	});

	// Standalone chain in arrow body (no assignment) - blank line mid-chain
	fn(() => {
		obj
			.method1()

			.method2();
	});

	// Arrow as callback argument - common pattern
	arr.map((x) => {
		const result = x
			.process()

			.transform();
		return result;
	});

	// Nested arrow functions
	fn(() => {
		return inner(() => {
			const result = a
				.method1()

				.method2();
		});
	});

	// Multiple statements with blank lines between
	fn(() => {
		const x = 1;

		const result = a
			.method1()

			.method2();

		return result;
	});

	// Optional chaining in arrow body
	fn(() => {
		const result = a
			?.method1()

			?.method2();
	});

	// Mixed member types - prettier keeps a.prop together and [key].method2() together
	fn(() => {
		const result = a.prop
			.method1()

			[key].method2();
	});

	// Blank line before FIRST method only (short, fits) - collapses to inline
	fn(() => {
		const result = a.method1().method2();
	});

	// Blank line before FIRST method only (exactly 100 chars) - collapses to inline
	fn(() => {
		const result000000000000000000000 = obj0000000000000000000000000000000.method1000().method200();
	});

	// Blank line before FIRST method only (exactly 101 chars) - preserves break
	fn(() => {
		const result0000000000000000000000 = obj0000000000000000000000000000000

			.method1000()
			.method200();
	});

	// Multiple blank lines
	fn(() => {
		const result = a
			.method1()

			.method2()

			.method3();
	});

	// Long chain that must break anyway - blank lines preserved
	fn(() => {
		const result = veryLongObjectName00000000000000000000
			.veryLongMethodName1()

			.veryLongMethodName2();
	});

	// Contrast: No blank lines - short chain stays inline
	fn(() => {
		const result = a.method1().method2();
	});

	// Contrast: No blank lines - long chain breaks normally
	fn(() => {
		const result = veryLongObjectName00000000000000000000
			.veryLongMethodName1()
			.veryLongMethodName2();
	});

	// IIFE - blank line forces break
	(function () {
		const result = a
			.method1()

			.method2();
	})();

	// Function expression - blank line forces break
	const myFn = function () {
		const result = a
			.method1()

			.method2();
	};

	// Arrow expression body - blank line forces break
	const expr = () =>
		a
			.method1()

			.method2();

	// Regular function declaration - blank line forces break
	function regularFn() {
		const result = a
			.method1()

			.method2();
	}

	// Class method - blank line forces break
	class Foo {
		bar() {
			const result = a
				.method1()

				.method2();
		}
	}
</script>
