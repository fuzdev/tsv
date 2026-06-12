<script lang="ts">
	// Optional method in abstract class
	abstract class A {
		abstract m?(): void;
		abstract withParam?(x: number): string;
	}

	// Optional method in declare class
	declare class C {
		m?(): void;
		// optional method returning a type predicate
		isA?(x: unknown): x is A;
	}

	// Optional method signature in interface (already supported)
	interface I {
		m?(): void;
	}
</script>
