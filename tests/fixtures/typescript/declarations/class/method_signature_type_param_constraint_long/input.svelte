<script lang="ts">
	class C {
		// 100 chars - signature fits, stays inline
		m<T extends string>(paramAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: T): {a: number} {}

		// 101 chars - constrained type param blocks param grouping: params break, object stays inline
		m<T extends string>(
			paramAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: T,
		): {a: number} {}
	}
</script>
