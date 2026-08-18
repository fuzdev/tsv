<script lang="ts">
	// A comment in a redundant instantiation-head pair leads the statement once the
	// pair strips.
	// c1
	fn<string>;
	const j = /* c2 */ /* c3 */ fn<string>;
</script>
