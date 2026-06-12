<script lang="ts">
	class A {
		// Intersection 92 chars (fits with property name)
		readonly #config: Required<Omit<BenchmarkConfig, 'on_iteration' | 'on_task_complete'>> &
			Pick<BenchmarkConfig, 'on_iteration' | 'on_task_complete'>;

		// Intersection 100 chars (exactly at boundary, still fits)
		property: GenericType<Type1, Type2, Type3, Type4, Type5, Type6aaaaaaaaaaaaaaaaaaaaaaaaaaaaaab> &
			SecondType;

		// Intersection 101 chars (over boundary, wraps inside generic)
		property: GenericType<
			Type1,
			Type2,
			Type3,
			Type4,
			Type5,
			Type6aaaaaaaaaaaaaaaaaaaaaaaaaaaaaabc
		> &
			SecondType;
	}
</script>
