<script lang="ts">
	class C {
		// Generic type arguments - 100 chars (stays inline)
		property: GenericType<TypeArgument1, TypeArgument2, TypeArgument3, TypeArgument4AAAAAAAAAAAAAA>;

		// Generic type arguments - 101 chars (wraps)
		property: GenericType<
			TypeArgument1,
			TypeArgument2,
			TypeArgument3,
			TypeArgument4AAAAAAAAAAAAAAA
		>;

		// Union type - 100 chars (stays inline)
		property: string | number | boolean | null | undefined | object | symbolAAAAAAAAAAAAAAAAAAAAAAA;

		// Union type - 101 chars (wraps)
		property:
			string | number | boolean | null | undefined | object | symbolAAAAAAAAAAAAAAAAAAAAAAAA;
	}
</script>
