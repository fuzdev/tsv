<script>
	let props = $state({});
	let x = $state(1);
	let v = $state('');
</script>

<div class:a={x} style:color={v} {...props}></div>
