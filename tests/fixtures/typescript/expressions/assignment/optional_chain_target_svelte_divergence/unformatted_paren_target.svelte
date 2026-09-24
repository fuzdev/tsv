<script lang="ts">
	(a?.b) = c;
	(a?.[0].c) += d;
	(a?.b) ??= c;
	[(a?.b), ...(a?.c)] = x;
	[(a?.b) = 1] = x;
	({ x: (a?.b) } = y);
	({ ...a?.b } = y);
	(a?.b!) = c;
	a?.b!.c = d;
</script>
