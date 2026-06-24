<script lang="ts">
	// A keyword inside a comment between a decorator and the real declaration
	// keyword (`class` / `export`) is comment text, not the keyword.
	@dec /* class */
	class C {}

	@dec /* export */
	export class D {}
</script>
