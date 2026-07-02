<script lang="ts">
	// Superclass call with explicit type arguments — the type-arg list closing
	// before `(` routes to the call, not the bare-superclass split
	class A extends mixin<T>(Base) {}
	const B = class extends mixin<T>(Base) {};

	// A generic-function-type argument — the arrow's `>` is not the list close
	class C extends mixin<() => void>(Base) {}
	const D = class extends mixin<(v: T) => R>(Base) {};

	// A comment inside the list is opaque, even one containing `>`
	class E extends mixin<T /* > */>(Base) {}

	// Contrast: no call after the list — the bare superclass keeps the type
	// arguments as its own type parameters
	class F extends Base<() => void> {}
</script>
