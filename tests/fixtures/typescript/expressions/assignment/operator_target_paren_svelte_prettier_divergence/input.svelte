<script lang="ts">
	(-a) = 1;
	(typeof a) += 1;
	(void a) = 1;
	(++a) = 1;
	(a++) ??= 1;
	(await a) = 1;
	(a + b) = 1;
	(a < b) = 1;
	(a in b) = 1;
	(a ?? b) = 1;
	[(-a) = 1] = x;
	({ a: (-b) = 1 } = x);
	fn(((-a) = 1));
	for ((-a) = 1; ;) {}
	`${((-a) = 1)}`;
	() => ((-a) = 1);
</script>
