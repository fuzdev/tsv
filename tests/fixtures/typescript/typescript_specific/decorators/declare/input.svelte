<script lang="ts">
	// Ambient class declaration
	@d
	declare class A {}

	// Ambient abstract class
	@d
	declare abstract class B {}

	// Decorator before `export declare`
	@d
	export declare class C {}

	// Decorator between `export` and `declare`
	export
	@d
	declare class D {}

	// Stacked decorators
	@d1
	@d2
	declare class E {}

	// Generic ambient class with a member
	@d
	declare class F<T> {
		a: T;
	}
</script>
