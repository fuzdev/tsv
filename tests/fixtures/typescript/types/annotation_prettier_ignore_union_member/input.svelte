<script lang="ts">
	// an own-line directive after the annotation's `:` stays own-line and freezes the
	// first union member (Rule A)
	let v:
		// prettier-ignore
		{x:   1} | c1;
</script>
