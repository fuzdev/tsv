<script lang="ts">
	const e = items.filter((x) => x).length; // after filter
	someLongName(() => 1).bbbbb(() => {
		f();
	}).ccccc; // c
	someLongName(() => {
		g();
	}).bbbbb(() => 2).ccccc; // c
	obj[fn().bar](); // c2
	const f = (() => fn().bar)[i]; // c3
</script>
