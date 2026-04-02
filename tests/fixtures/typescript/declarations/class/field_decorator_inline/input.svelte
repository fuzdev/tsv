<script lang="ts">
	class A {
		// Single decorator, inline
		@decorator prop = 1;

		// Decorator with type annotation
		@decorator prop2: string = 'value';

		// Call expression decorator
		@decorator() prop3 = 1;

		// Multiple decorators, inline
		@dec1 @dec2 prop4 = 1;

		// Static field with decorator
		@decorator static prop5 = 1;

		// Readonly field with decorator
		@decorator readonly prop6 = 1;

		// Private field with decorator
		@decorator private prop7 = 1;

		// Method with decorator
		@decorator method() {}
	}
</script>
