<script>
	for (a = (b in c); d; e) {}
	for (a = cond ? (b in c) : 0; d; e) {}
	for (
		C = class {
			get [(b in c)]() {}
		};
		d;
		e
	) {}
	for (
		a = function () {
			z = (b in c);
		};
		d;
		e
	) {}
	for (f((a in b)); d; e) {}
	for (new C((a in b)); d; e) {}
</script>
