<script lang="ts">
	// Optional destructuring-pattern parameters in signature and ambient contexts.
	interface I {
		// array and object patterns
		m( [] ? ): void;
		n( {} ? ): void;
		// typed optional pattern, and an optional pattern in second position
		o( [ a ] ? : number[] ): void;
		p( {} , [ b ] ? ): any;
		// call and construct signatures
		( [] ? ): void;
		new ( {} ? ): void;
	}

	// Type-literal method
	type T = {
		m( [] ? ): void;
	};

	// Ambient declarations
	declare function f( [] ? ): void;
	declare class C {
		m( {} ? ): void;
	}
</script>
