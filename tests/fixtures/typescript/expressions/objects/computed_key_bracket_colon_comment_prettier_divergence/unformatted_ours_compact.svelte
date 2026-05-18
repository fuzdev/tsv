<script>
	const a = {[x]/* c */: 1};

	const b = {
		[x]/* c */: 1,
		[x]/* c */() {},
		async [x]/* c */() {},
		*[x]/* c */() {},
		get [x]/* c */() {
			return 1;
		},
		set [x]/* c */(a) {},
	};
</script>
