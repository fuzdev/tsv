<script lang="ts">
	// Function declaration
	async /* a */ function a() {}

	// Function expressions — named, anonymous, generator
	const b = async /* b */ function named() {};
	const c = async /* c */ function () {};
	const d = async /* d */ function* () {};

	// Overload signature (a bodiless declaration prints through its own builder)
	async /* e */ function e(): Promise<void>;
	async function e(x?: number): Promise<void> {}

	// `declare` is the other modifier that can open the head
	declare /* f */ function f(): void;
</script>
