<script lang="ts">
	// Class declaration
	class A// c1
	<T> {}

	// Class expression
	const a = class B// c2
	<T> {};

	// Interface
	interface C// c3
	<T> {}

	// Type alias
	type D// c4
	<T> = T;

	// Function
	function fn1// c5
	<T>() {}
</script>
