<script lang="ts">
	// type-member signatures cannot carry parameter decorators; plain signatures are valid
	interface I {
		m(a: number): void;
		(a: number): void;
		new (a: number): void;
		set x(a: number);
	}
	type T = { m(a: number): void };
</script>
