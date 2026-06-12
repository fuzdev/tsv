<script lang="ts">
	/* object arg stays hugged */
	const a = Obj.extend({
		a: 1,
		b: 2,
	}).fn({prop: 'a'});

	/* array arg stays hugged */
	const b = Arr.from([1, 2, 3]).map((x) => x * 2);

	/* multiple object chains */
	const c = Obj.create({
		prop: fn(),
	})
		.fn1()
		.fn2({prop: 'c'});

	/* short object - stays inline */
	const d = Obj.extend({a: 1, b: 2}).fn({prop: 'd'});

	/* short chain - stays inline */
	const e = Obj.create({prop: fn()}).fn1().fn2({prop: 'e'});
</script>
