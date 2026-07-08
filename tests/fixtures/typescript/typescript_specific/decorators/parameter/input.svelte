<script lang="ts">
	class A {
		// Simple parameter decorator
		fn1(@decorator a: number) {}

		// Multiple decorators
		fn2(@dec1 @dec2 a: number) {}

		// With expression
		fn3(@dec(1) a: number) {}

		// With default value (decorators attach to the AssignmentPattern)
		fn4(@decorator a = 1) {}

		// Constructor parameter
		constructor(@decorator a: number) {}
	}
</script>
