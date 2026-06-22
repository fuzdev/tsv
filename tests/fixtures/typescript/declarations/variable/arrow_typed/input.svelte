<script lang="ts">
	// Short - fits on one line
	const a: (x: number) => void = (x) => {};

	// 100 chars - at boundary, arrow stays on line 1
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: (x: number, y: string, z: boolean) => void = (x, y, z) => {
		console.log(x);
	};

	// 101 chars - exceeds, arrow PARAMS wrap (not after =)
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: (x: number, y: string, z: boolean) => void = (
		x,
		y,
		z
	) => {
		console.log(x);
	};
</script>
