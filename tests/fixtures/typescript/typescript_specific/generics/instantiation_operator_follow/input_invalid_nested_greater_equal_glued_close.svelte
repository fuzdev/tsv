<script lang="ts">
	// The inner close sits in a type, where `>` is always one token; the OUTER close is
	// in the expression, where `>=` is. So the source reads `f < A < B >>= c` — the
	// outer list never closes, and neither does the one-level-shorter `f < A<B>` reading.
	f<A<B>>=c;
</script>
