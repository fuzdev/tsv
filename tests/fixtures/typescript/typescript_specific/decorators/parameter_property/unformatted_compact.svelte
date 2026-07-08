<script lang="ts">
	class A {
		constructor(@dec private a: number, @dec readonly b: number, @dec private readonly c: number, @dec(1) protected d: number, @dec public e: number = 1) {}
	}
</script>
