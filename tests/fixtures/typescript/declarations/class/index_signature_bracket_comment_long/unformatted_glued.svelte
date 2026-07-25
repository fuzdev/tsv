<script lang="ts">
	// 100 chars flat - the key pulls up onto the comment's line
	class A {
		[/* aAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA */
		k: string]: T;
	}
	// 101 chars flat - the key MUST stay below the comment and the bracket breaks
	class B {
		[/* aAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA */
		k: string]: T;
	}
</script>
