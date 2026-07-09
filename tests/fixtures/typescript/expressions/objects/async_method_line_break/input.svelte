<script lang="ts">
	const o = {
		// async method — the `async` modifier is on the same line (valid)
		async m() {},
		// get/set carry no [no LineTerminator here] restriction (valid regardless)
		get x() {},
		set y(v) {},
		// `async` as an ordinary property key (valid)
		async: 1,
		p: 2
	};
</script>
