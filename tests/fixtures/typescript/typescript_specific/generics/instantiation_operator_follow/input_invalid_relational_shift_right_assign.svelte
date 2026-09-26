<script lang="ts">
	// A spaced `<` is no different: the first `>` of `>>=` is not a close, so this is the
	// comparison `a < b` assigned to, not `(a<b>) >= c`.
	a < b >>= c;
</script>
