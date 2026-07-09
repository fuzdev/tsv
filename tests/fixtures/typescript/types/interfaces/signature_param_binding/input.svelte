<script lang="ts">
	interface I {
		// a signature param may be plain, typed, destructuring, or rest — valid
		m(a: number, [b], { c }, ...d: number[]): void;
		// call and construct signatures follow the same param grammar — valid
		(x: number): number;
		new (x: number): I;
	}
</script>
