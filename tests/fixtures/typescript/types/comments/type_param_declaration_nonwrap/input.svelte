<script lang="ts">
	// Line comment in call signature type params (non-wrapping path)
	type A = {
		<
			T // c
		>(): void;
	};

	// Block comment in call signature type params
	type B = {
		<T /* c */>(): void;
	};

	// Line comment in construct signature type params
	type C = {
		new <
			T // c
		>(): C;
	};
</script>
