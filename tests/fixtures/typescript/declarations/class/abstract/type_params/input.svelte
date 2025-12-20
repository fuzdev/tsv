<script lang="ts">
	// Single type parameter
	abstract class Single<T> {
		abstract value: T;
		abstract fn(x: T): T;
	}

	// Multiple type parameters
	abstract class Multiple<T, U> {
		abstract a: T;
		abstract b: U;
	}

	// With constraint
	abstract class WithConstraint<T extends object> {
		abstract items: T[];
		abstract add(item: T): void;
	}

	// With default type
	abstract class WithDefault<T = string> {
		abstract value: T;
	}

	// Abstract extending generic
	abstract class Extends<T> extends Single<T> {
		abstract other: T;
	}

	// Concrete implementing generic abstract
	class Concrete<T> extends Single<T> {
		value: T;
		constructor(value: T) {
			super();
			this.value = value;
		}
		fn(x: T): T {
			return x;
		}
	}
</script>
