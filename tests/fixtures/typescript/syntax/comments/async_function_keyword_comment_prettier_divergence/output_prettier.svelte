<script lang="ts">
	// Function declaration
	async function /* a */ a() {}

	// Function expressions — named, anonymous, generator
	const b = async function /* b */ named() {};
	const c = async function () /* c */ {};
	const d = async function* () /* d */ {};

	// Overload signature (a bodiless declaration prints through its own builder)
	async function /* e */ e(): Promise<void>;
	async function e(x?: number): Promise<void> {}

	// `declare` is the other modifier that can open the head
	declare function /* f */ f(): void;
</script>
