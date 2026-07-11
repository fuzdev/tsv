<script lang="ts">
	class Foo extends (
		@deco
		class Foo {}
	) {}
	class Bar extends (
		@deco
		class {}
	) {}
</script>
