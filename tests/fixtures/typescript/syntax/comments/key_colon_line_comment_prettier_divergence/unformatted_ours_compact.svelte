<script lang="ts">
	interface I {
		a// c1
		:number;
	}

	type T = {
		b// c2
		:number;
	};

	class A {
		c// c3
		:number=1;
	}
</script>
