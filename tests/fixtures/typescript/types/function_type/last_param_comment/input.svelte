<script lang="ts">
	// block comment trailing the last param is preserved in place
	type F = (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: number,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: number /* comment */
	) => void;
</script>
