<script>
	/* Short chain - stays inline (under 100 chars) */
	const short = arr.filter((x)=>x>0).map((x)=>x*2);

	/* Long chain - exceeds 100 chars, wraps to one call per line */
	const long = arr.filter((x)=>x.val).map((x)=>x.str).sort((a,b)=>a.localeCompare(b)).join(', ');
</script>
