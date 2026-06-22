<script lang="ts">
	// Arrow function - inline comment
	const fn1 = (
		a: string // comment
	): void => {};

	// Arrow function - comment on separate line
	const fn2 = (
		a: string
		// comment
	): void => {};

	// Regular function declaration
	function fn3(
		a: string // comment
	): void {}

	// Object method
	const obj = {
		fn(
			a: string // comment
		): void {}
	};

	// Class method
	class A {
		fn(
			a: string // comment
		): void {}
	}

	// Constructor
	class B {
		constructor(
			a: string // comment
		) {}
	}

	// Multiple params - comment on last
	const fn4 = (
		a: string,
		b: number // comment
	): void => {};

	// Block comment variant
	const fn5 = (a: string /* comment */): void => {};
</script>
