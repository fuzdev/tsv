<script lang="ts">
	// an own-line directive after `label:` stays own-line and freezes the element type
	type A = [
		label:
				// prettier-ignore
				{x:   1},
		other:  b
	];
</script>
