<script lang="ts">
	// A parenthesized type takes an index or array suffix, never a qualified-name
	// one, so `(B).C` has no reading.
	const f = (a: A): a is (B).C => true;
</script>
