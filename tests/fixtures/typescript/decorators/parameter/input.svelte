<script lang="ts">
	class A {
		// Simple parameter decorator
		fn1(@decorator a: number) {}

		// Multiple decorators
		fn2(@dec1 @dec2 a: number) {}

		// With expression
		fn3(@dec(1) a: number) {}

		// Constructor parameter
		constructor(@decorator a: number) {}
	}
</script>
