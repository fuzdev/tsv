<script lang="ts">
	// Method signature with complex return type - params should hug method name
	class Single {
		fn(graph: T): {
			a: Array<Array<string>>;
			b: Array<Array<string>>;
			c: Array<{x: string; y: string; z: string}>;
		} {
			return {a: [], b: [], c: []};
		}
	}
</script>
