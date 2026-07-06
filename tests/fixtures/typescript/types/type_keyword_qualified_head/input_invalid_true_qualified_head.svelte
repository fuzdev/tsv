<script lang="ts">
	// `true`/`false` are literal types, not entity-name heads: a qualified `.X`
	// after them is rejected (unlike the contextual keyword heads above)
	type Q = true.X;
</script>
