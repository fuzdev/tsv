<script lang="ts">
	// block comment after the last type parameter's comma is preserved in place
	function fn<
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, /* comment */
	>() {}
</script>
