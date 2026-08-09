<script lang="ts">
	declare namespace N {
		function f() {}
	}

	declare module M {
		function g() {}
	}

	declare namespace E {
		export function h() {}
	}
</script>
