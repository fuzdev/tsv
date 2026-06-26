<script lang="ts">
	class A extends (new Base()) {}
	class B extends (new Base<T>()) {}
	class C extends (tag`t`) {}
	class D extends (Base!) {}
</script>
