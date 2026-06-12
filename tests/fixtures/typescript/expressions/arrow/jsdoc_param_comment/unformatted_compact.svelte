<script>
	// Single call arg
	const a = items.map(/** @param {any} x */(x)=>fn(x));

	// Second call arg
	var b = obj.observe(element,/** @param {any} entry */(entry)=>set(entry));

	// Block body - comment forces call arg expansion
	const c = new Observer(/** @param {any} entries */(entries)=>{
			for (var entry of entries) {
				process(entry);
			}
		},
	);
</script>
