<script lang="ts">
	// block comment after the last param's comma is preserved in place
	type F = (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: number,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: number /* comment */,
	) => void;
</script>
