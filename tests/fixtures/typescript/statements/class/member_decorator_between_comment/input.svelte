<script lang="ts">
	class A {
		@fn1
		// c1
		@fn2
		a = 1;

		@fn1
		// c2
		// c3
		@fn2
		b = 1;
	}
</script>
