<script lang="ts">
	const e = items.filter((x) => x) // after filter
		.length;
	someLongName(() => 1).bbbbb(() => {
		f();
	}) // c
		.ccccc;
	someLongName(() => {
		g();
	}).bbbbb(() => 2) // c
		.ccccc;
	obj[
		fn() // c2
			.bar
	]();
	const f = (() =>
		fn() // c3
			.bar)[i];
</script>
