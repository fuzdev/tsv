<script lang="ts">
	// Simple construct signature
	type A = new () => B;

	// Generic construct signature
	type C = new <T>(a: T) => T;

	// Abstract construct signature
	type D = abstract new () => E;

	// Abstract generic construct signature
	type F = abstract new <T>(a: T) => T;
</script>
