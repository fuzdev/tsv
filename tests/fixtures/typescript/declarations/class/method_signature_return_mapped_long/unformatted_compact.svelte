<script lang="ts">
	class C {
		// 100 chars - signature fits, stays inline
		m(paramAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: string): {[K in keyof T]: T[K]} {}

		// 101 chars - signature breaks: single param stays flat, mapped return type expands
		m(paramAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: string): {[K in keyof T]: T[K]} {}
	}
</script>
