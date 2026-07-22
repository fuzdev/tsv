<script>
	let props = $state({});
	let x = $state(1);
</script>

<div class="base" class:a={x} {...props}></div>
