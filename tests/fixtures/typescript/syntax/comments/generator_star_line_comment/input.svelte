<script>
	class A {
		*// c1
		a() {}
		async *// c2
		b() {}
		*// c3
		#c() {}
		*/* c4 */ // c5
		d() {}
		*// c6
		/* c7 */ e() {}
		*// c8
		// c9
		f() {}
	}

	const o = {
		*// c10
		a() {},
		async *// c11
		b() {}
	};
</script>
