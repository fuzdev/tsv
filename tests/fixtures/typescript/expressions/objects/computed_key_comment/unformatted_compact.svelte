<script>
	let a = {[x/* c */]: 1};

	let b = {
		[x/* c */]() {},
		async [x/* c */]() {},
		*[x/* c */]() {},
		get [x/* c */]() {
			return 1;
		},
		set [x/* c */](a) {},
	};
</script>
