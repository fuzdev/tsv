<script lang="ts">
	interface WithGenericMethod {
		method<T>(x: T): T;
	}

	interface MultipleGenerics {
		transform<T, U>(input: T): U;
	}

	interface ConstrainedGeneric {
		parse<T extends string>(value: T): T;
	}

	interface OptionalGenericMethod {
		maybe?<T>(x: T): T | undefined;
	}
</script>
