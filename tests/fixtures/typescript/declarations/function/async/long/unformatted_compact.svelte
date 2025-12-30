<script lang="ts">
	// Short async - stays on one line (contrast case)
	async function short(): Promise<void> {}

	// Long async function signature - params wrap
	async function longParams(veryLongParameterNameOne: VeryLongTypeName, veryLongParameterNameTwo: AnotherLongTypeName, veryLongParameterNameThree: YetAnotherLongTypeName): Promise<void> { await Promise.resolve(); }

	// Long async with generic - type params wrap
	async function longGeneric<T extends VeryLongConstraintTypeName, U extends AnotherLongConstraintTypeName>(x: T, y: U): Promise<[T, U]> { return [x, y]; }

	// Async generator with long signature
	async function* longAsyncGenerator(veryLongParameterNameOne: VeryLongTypeName, veryLongParameterNameTwo: AnotherLongTypeName): AsyncGenerator<VeryLongYieldTypeName> { yield await Promise.resolve({} as VeryLongYieldTypeName); }

	// Long async arrow function
	const longArrow = async (veryLongParameterNameOne: VeryLongTypeName, veryLongParameterNameTwo: AnotherLongTypeName): Promise<void> => { await Promise.resolve(); };

	// Long async method in object
	const obj = { async longMethod(veryLongParameterNameOne: VeryLongTypeName, veryLongParameterNameTwo: AnotherLongTypeName): Promise<void> { await Promise.resolve(); } };

	// Exported async with long signature
	export async function exportedLong(veryLongParameterNameOne: VeryLongTypeName, veryLongParameterNameTwo: AnotherLongTypeName): Promise<VeryLongReturnTypeName> { return {} as VeryLongReturnTypeName; }
</script>
