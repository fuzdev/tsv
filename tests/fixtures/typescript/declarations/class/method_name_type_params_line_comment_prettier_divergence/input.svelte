<script lang="ts">
	// Class method
	class A {
		fn1 // c1
		<T>() {}
	}

	// Object method
	const b = {
		fn1 // c2
		<T>() {},
	};

	// Interface method
	interface C {
		fn1 // c3
		<T>(): void;
	}
</script>
