<script lang="ts">
	// an own-line directive after `as` stays own-line and freezes the type
	const a = value as // prettier-ignore
	{x:   1};

	// satisfies host
	const b = value satisfies // prettier-ignore
	{x:   2};
</script>
