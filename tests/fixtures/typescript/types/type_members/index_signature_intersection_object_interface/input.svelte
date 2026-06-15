<script lang="ts">
	interface I {
		[key: string]: A & {
			b: B;
		};
	}

	interface U {
		[k: number]: {
			a: A;
			b: B;
		} & Base;
	}
</script>
