<script lang="ts">
	// Simple decorator
	@d
	class A {}

	// Decorator with call expression
	@d()
	class WithCall {}

	// Decorator with arguments
	@d('value')
	class WithArgs {}

	// Multiple decorators (stacked)
	@d1()
	@d2
	class Stacked {}

	// Decorator factory with complex argument
	@d({a: 'value', b: true})
	class WithObject {}

	// Exported decorated class
	@d
	export class Exported {}

	// Default exported decorated class
	@d
	export default class DefaultExported {}

	// Decorated class with generic
	@d
	class Generic<T> {
		value: T;
	}

	// Decorated class extending another
	@d
	class Extended extends Base {
		fn() {}
	}

	// Member access decorator expression
	@d.nested
	class MemberAccess {}
</script>
