<script lang="ts">
	class A {
		// Comment between name and ? (no type annotation)
		a? /* c1 */ = 1;

		// Comment between name and ! (no type annotation)
		b! /* c2 */ = 1;
	}
</script>
