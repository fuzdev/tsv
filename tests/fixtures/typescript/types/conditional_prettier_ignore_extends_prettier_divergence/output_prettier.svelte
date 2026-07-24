<script lang="ts">
	// an own-line directive after `extends` stays own-line and freezes the extends type
	type A = X extends // prettier-ignore
	{x:   1}
		? a
		: b;
</script>
