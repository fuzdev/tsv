<script lang="ts">
	declare class Generic {
		// Single type parameter
		single<T>(x: T): T;

		// Multiple type parameters
		multiple<T, U>(x: T, y: U): [T, U];

		// Constrained type parameter
		constrained<T extends A>(x: T): T;

		// Static generic method
		static staticGeneric<T>(x: T): T;

		// Generic method returning a type predicate
		predicate<T>(x: unknown): x is T;
	}
</script>
