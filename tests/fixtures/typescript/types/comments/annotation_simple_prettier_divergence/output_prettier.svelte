<script lang="ts">
	interface A {
		a: X; // simple
		b?: X; // optional
		readonly c: X; // readonly
		['d']: X; // computed
		e: Array<X>; // generic
	}

	type B = {
		f: X; // leading // trailing
		g: // c2 // c1
		X;
		h: X; /* trailing */ // leading
	};
</script>
