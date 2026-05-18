<script lang="ts">
	// Class declaration
	class A<T> {} // c1

	// Class expression
	const a = class B<T> {}; // c2

	// Interface
	interface C<T> {} // c3

	// Type alias
	type D<T> = // c4
		T;

	// Function
	function fn1<T>() {} // c5
</script>
