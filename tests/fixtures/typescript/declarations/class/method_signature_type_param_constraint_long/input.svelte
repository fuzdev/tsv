<script lang="ts">
	class C {
		// 100 chars - signature fits, stays inline
		m<T extends string>(paramAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: T): { a: number } {}

		// 101 chars - constrained type param blocks param grouping: params break, object stays inline
		m<T extends string>(
			paramAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: T
		): { a: number } {}
	}
</script>
