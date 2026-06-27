<script lang="ts">
	const C =
		@dec
		class {};
	const D =
		@dec.foo
		class extends Base {};
	const E =
		@dec()
		class {
			a = 1;
		};

	// Multiple decorators
	const M =
		@a
		@b
		class {};

	// Non-assignment position: decorators render, but no break after `=`
	foo(
		@dec
		class {}
	);
</script>
