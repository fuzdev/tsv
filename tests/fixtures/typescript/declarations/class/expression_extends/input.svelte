<script lang="ts">
	// Anonymous class expression with extends
	const A = class extends Base {};
	const B = class extends Base {
		method() {
			return super.method();
		}
	};

	// Named class expression with extends
	const C = class Named extends Base {};
	const D = class Named extends Base {
		constructor() {
			super();
		}
	};

	// Class expression with extends and implements
	const E = class extends Base implements Iface {};
	const F = class Named extends Base implements Iface1, Iface2 {
		prop: string;
	};

	// Class expression with generic base
	const G = class extends Generic<T> {};
	const H = class<T> extends Generic<T> {
		value: T;
	};

	// Nested class expressions
	const Outer = class extends Base {
		Inner = class extends Base2 {};
	};

	// Class expression assigned directly
	x = class extends Base {};

	// Complex extends expression
	const Mixin = class extends getMixin(Base) {};

	// Chained after mixin call
	const Chained = class extends getMixin(Base).Inner {};

	// Type arguments on mixin call
	const TypedMixin = class extends getMixin<T>(Base) {};
	const MultiTyped = class extends createMixin<T, U>(Base, Config) {};
</script>
