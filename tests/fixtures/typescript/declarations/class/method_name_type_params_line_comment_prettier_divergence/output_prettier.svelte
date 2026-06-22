<script lang="ts">
	// Class method
	class A {
		fn1<T>() {} // c1
	}

	// Object method
	const b = {
		fn1<T>() {} // c2
	};

	// Interface method
	interface C {
		fn1<T>(): void; // c3
	}
</script>
