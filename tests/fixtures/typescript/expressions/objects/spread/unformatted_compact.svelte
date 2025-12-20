<script>
	const spread = {...other};
	const before = {...other,a:1};
	const after = {a:1,...other};
	const middle = {a:1,...other,b:2};
	const multiple = {...obj1,...obj2};
	const mixed = {a,...other,b:1};
</script>
