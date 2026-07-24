<script lang="ts">
	// an own-line directive inside a required paren freezes the inner function type only
	type A = (
		// prettier-ignore
		(a:  string) => void
	)[];
</script>
