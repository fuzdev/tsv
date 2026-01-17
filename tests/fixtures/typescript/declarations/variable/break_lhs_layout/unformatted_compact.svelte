<script lang="ts">
	// Complex type annotation - Map with nested Array
	const myMap: Map<string,Array<number>> = getSomeVeryLongValueThatWillDefinitelyExceedThePrintWidthLimit();

	// Complex type annotation - Record with nested Array
	const myRecord: Record<string,Array<ComplexType>> = getVeryLongValueNameThatExceedsPrintWidthLimit();

	// Conditional type as type parameter
	const value: Conditional<T extends string ? number : boolean,OtherType> = getVeryLongValueThatExceeds();

	// Type alias with constrained type params
	type MyVeryLongTypeName<T extends string,U = number> = SomeVeryLongTypeNameThatDefinitelyExceedsPrintWidthLimit;
</script>
