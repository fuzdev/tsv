<script lang="ts">
	interface A {
		x: number;
	}

	interface A {
		y: string;
	}

	interface A {
		z: boolean;
	}

	const a: A = {x: 1, y: 'a', z: true};
</script>
