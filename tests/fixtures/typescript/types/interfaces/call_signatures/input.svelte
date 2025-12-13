<script lang="ts">
	// Call signature (generic)
	interface Callable {
		<T>(): T;
		<T>(x: T): T;
		<T, U>(x: T, y: U): [T, U];
	}

	// Construct signature (generic)
	interface Constructable {
		new <T>(): T;
		new <T>(x: T): T;
	}

	// Mixed call and construct signatures
	interface Mixed {
		(): void;
		<T>(): T;
		new (): Mixed;
		new <T>(x: T): T;
	}

	// Type literal with call signature
	type Fn = {
		<T>(): T;
	};
</script>
