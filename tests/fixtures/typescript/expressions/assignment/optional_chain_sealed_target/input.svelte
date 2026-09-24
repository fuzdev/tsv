<script lang="ts">
	// parens seal the chain, so each target is an ordinary member access
	(a?.b).c = d;
	(a?.[0])[c] += d;
	[(a?.b).c] = x;
	({ x: (a?.b).c } = y);
	(a?.b).c++;
</script>
