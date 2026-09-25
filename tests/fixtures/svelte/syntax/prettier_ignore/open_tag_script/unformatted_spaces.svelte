<script>   // prettier-ignore
	const   a = 1;
	c;   // prettier-ignore
	d   =   1;
</script>

<!-- a directive opening the body, glued to the open tag or not, freezes the next statement -->
<div>
	<script>   // prettier-ignore
		const   e = 1;
	</script>
	<script>   /* prettier-ignore */
		const   f = 1;
	</script>
</div>
