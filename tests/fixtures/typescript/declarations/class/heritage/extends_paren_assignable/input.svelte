<script lang="ts">
	class A extends (a = b) {}
	class B extends (await x) {}
	class C extends ((a) => b) {}
	class D extends (a as T) {}
	class E extends (-a) {}
	class F extends ({}) {}
</script>
